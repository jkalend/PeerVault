#!/bin/sh
# Hard-power-off drill for a Linux guest. Run on a host with QEMU, a kernel,
# an initrd and network access for the Ubuntu base rootfs.
set -eu

PEERVAULT=""
RESTIC=""
WORK="/tmp/pv-powerloss"
FILL_MIB="256"
ROOTFS_URL="https://cdimage.ubuntu.com/ubuntu-base/releases/24.04/release/ubuntu-base-24.04.3-base-amd64.tar.gz"

usage() {
    echo "usage: $0 --peervault <bin> --restic <bin> [--work <dir>] [--fill-mib N] [--rootfs-url URL]" >&2
    exit 2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --peervault) PEERVAULT="$2"; shift 2 ;;
        --restic) RESTIC="$2"; shift 2 ;;
        --work) WORK="$2"; shift 2 ;;
        --fill-mib) FILL_MIB="$2"; shift 2 ;;
        --rootfs-url) ROOTFS_URL="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "unknown argument: $1" >&2; usage ;;
    esac
done
[ -x "$PEERVAULT" ] || { echo "peervault binary is not executable: $PEERVAULT" >&2; exit 1; }
[ -x "$RESTIC" ] || { echo "restic binary is not executable: $RESTIC" >&2; exit 1; }
[ -x /bin/busybox ] || { echo "busybox-static is required at /bin/busybox" >&2; exit 1; }
command -v qemu-system-x86_64 >/dev/null || { echo "qemu-system-x86_64 is required" >&2; exit 1; }
command -v mkfs.ext4 >/dev/null || { echo "e2fsprogs is required" >&2; exit 1; }
KERNEL="$(ls /boot/vmlinuz-* 2>/dev/null | head -1 || true)"
INITRD="$(ls /boot/initrd.img-* 2>/dev/null | head -1 || true)"
[ -n "$KERNEL" ] && [ -n "$INITRD" ] || { echo "kernel/initrd not found under /boot" >&2; exit 1; }

rm -rf "$WORK"
mkdir -p "$WORK/rootfs"
wget -qO- "$ROOTFS_URL" | tar -xz -C "$WORK/rootfs"
cp /bin/busybox "$WORK/rootfs/bin/busybox"
cp "$PEERVAULT" "$WORK/rootfs/peervault"
cp "$RESTIC" "$WORK/rootfs/restic"
chmod 755 "$WORK/rootfs/bin/busybox" "$WORK/rootfs/peervault" "$WORK/rootfs/restic"
ldd "$PEERVAULT" | awk '/=> \// {print $3} /^\/.*ld-linux/ {print $1}' | while read -r library; do
    [ -f "$library" ] || continue
    mkdir -p "$WORK/rootfs$(dirname "$library")"
    cp "$library" "$WORK/rootfs$library"
done
mkdir -p "$WORK/rootfs/etc"
cp /etc/ld.so.cache "$WORK/rootfs/etc/ld.so.cache" 2>/dev/null || true
printf 'hard-poweroff-drill-secret\n' > "$WORK/rootfs/secret"
chmod 600 "$WORK/rootfs/secret"
mkdir -p "$WORK/rootfs/repo" "$WORK/rootfs/source"
"$PEERVAULT" init --repository "$WORK/rootfs/repo" --backend "$RESTIC" --password-file "$WORK/rootfs/secret" >/dev/null

cat > "$WORK/rootfs/init" <<'INIT'
#!/bin/busybox sh
/bin/busybox --install -s
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
export PEERVAULT_PASSWORD_FILE=/secret
if [ ! -f /phase2 ]; then
    echo baseline > /source/baseline.txt
    /peervault capture --repository /repo --backend /restic --source /source --password-file /secret > /baseline.json || { echo BASELINE_FAIL; poweroff -f; }
    snapshot=$(sed -n 's/.*"snapshot_id": "\([0-9a-f]*\)".*/\1/p' /baseline.json | head -1)
    [ -n "$snapshot" ] || { echo BASELINE_FAIL; poweroff -f; }
    echo "$snapshot" > /baseline.id
    head -c $((FILL_COUNT * 1024 * 1024)) /dev/urandom > /source/fill.bin
    [ -s /source/fill.bin ] || { echo FILL_FAIL; poweroff -f; }
    sync
    touch /phase2
    sync
    echo PHASE1_READY
    /peervault capture --repository /repo --backend /restic --source /source --password-file /secret > /second.json
    echo CAPTURE_UNEXPECTED_SUCCESS
    poweroff -f
else
    /peervault history --repository /repo --backend /restic --password-file /secret > /history.json || echo HISTORY_FAIL
    /peervault verify --repository /repo --backend /restic --password-file /secret > /verify.json || echo VERIFY_FAIL
    /peervault restore --repository /repo --backend /restic --snapshot "$(cat /baseline.id)" --destination /restore-target --password-file /secret > /restore.json || echo RESTORE_FAIL
    if [ -f /restore-target/baseline.txt ] && [ ! -f /restore-target/fill.bin ]; then
        echo RESULT_OK
    else
        echo RESULT_FAIL
    fi
    poweroff -f
fi
INIT
sed -i "s/FILL_COUNT/$FILL_MIB/" "$WORK/rootfs/init"
chmod 755 "$WORK/rootfs/init"
mkfs.ext4 -q -F -d "$WORK/rootfs" "$WORK/disk.img" 1024M

run_qemu() {
    log="$1"
    qemu-system-x86_64 -machine pc,accel=tcg -cpu max -smp 2 -m 512 \
        -drive file="$WORK/disk.img",format=raw,if=ide,cache=none \
        -kernel "$KERNEL" -initrd "$INITRD" \
        -append "root=/dev/sda rw console=ttyS0 init=/init panic=-1" \
        -nographic -no-reboot >"$log" 2>&1 &
    QEMU_PID=$!
}

wait_for() {
    pattern="$1" log="$2" limit="$3"
    count=0
    while [ "$count" -lt "$limit" ]; do
        if grep -q "$pattern" "$log" 2>/dev/null; then return 0; fi
        count=$((count + 1))
        sleep 1
    done
    return 1
}

run_qemu "$WORK/serial1.log"
qemu="$QEMU_PID"
if ! wait_for PHASE1_READY "$WORK/serial1.log" 180; then
    kill -9 "$qemu" 2>/dev/null || true
    tail -30 "$WORK/serial1.log"
    echo "guest did not reach the capture phase" >&2
    exit 1
fi
sleep 2
kill -9 "$qemu" 2>/dev/null || true
sleep 1
if ps -p "$qemu" >/dev/null 2>&1; then
    echo "QEMU $qemu survived SIGKILL" >&2
    exit 1
fi
echo "power-loss: QEMU $qemu killed during the second capture" >&2

run_qemu "$WORK/serial2.log"
qemu="$QEMU_PID"
if ! wait_for RESULT_OK "$WORK/serial2.log" 180; then
    kill -9 "$qemu" 2>/dev/null || true
    tail -40 "$WORK/serial2.log"
    echo "guest did not recover the baseline after power loss" >&2
    exit 1
fi
wait "$qemu" 2>/dev/null || true
printf '{"result":"pass","fill_mib":%s,"power_loss":"QEMU kill -9 during capture","baseline_restored":true,"limitation":"virtual IDE disk, TCG, Ubuntu container rootfs; not physical hardware"}\n' "$FILL_MIB"
