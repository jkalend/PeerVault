using System.Diagnostics;
using System.IO.Pipes;
using System.Text;
using System.Text.Json;

namespace PeerVault.Desktop;

public sealed class Form1 : Form
{
    private readonly TextBox _core = new() { Text = "peervault.exe", Width = 520 };
    private readonly TextBox _endpoint = new() { Width = 520 };
    private readonly TextBox _config = new() { Width = 520 };
    private readonly TextBox _state = new() { Width = 520 };
    private readonly TextBox _repository = new() { Width = 520 };
    private readonly TextBox _backend = new() { Text = "restic.exe", Width = 520 };
    private readonly TextBox _source = new() { Width = 520 };
    private readonly TextBox _password = new() { Width = 520 };
    private readonly TextBox _snapshot = new() { Width = 520 };
    private readonly TextBox _destination = new() { Width = 520 };
    private readonly TextBox _policy = new() { Width = 520 };
    private readonly RichTextBox _output = new() { ReadOnly = true, Dock = DockStyle.Fill, Font = new Font("Consolas", 9) };
    private readonly Button _status = new() { Text = "Status", Width = 110 };
    private readonly Button _tick = new() { Text = "Run due jobs", Width = 110 };
    private readonly Button _capture = new() { Text = "Capture", Width = 110 };
    private readonly Button _history = new() { Text = "History", Width = 110 };
    private readonly Button _verify = new() { Text = "Verify", Width = 110 };
    private readonly Button _restore = new() { Text = "Restore", Width = 110 };
    private readonly Button _preview = new() { Text = "Retention preview", Width = 130 };
    private Process? _agent;
    private NamedPipeClientStream? _pipe;
    private StreamWriter? _pipeInput;
    private StreamReader? _pipeOutput;
    private long _requestId;

    public Form1()
    {
        _endpoint.Text = $"peervault-desktop-{Environment.ProcessId}";
        Text = "PeerVault owner desktop (experimental)";
        Width = 980;
        Height = 760;
        var labels = new[]
        {
            ("Core", _core), ("Agent pipe", _endpoint), ("Jobs config", _config),
            ("Scheduler state", _state), ("Repository", _repository), ("Backend", _backend),
            ("Source", _source), ("Password file", _password), ("Snapshot", _snapshot),
            ("Destination", _destination), ("Retention policy", _policy),
        };
        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = labels.Length + 2,
            Padding = new Padding(8),
            AutoScroll = true,
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 110));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        for (var row = 0; row < labels.Length; row++)
        {
            layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 32));
            layout.Controls.Add(new Label { Text = labels[row].Item1, AutoSize = true }, 0, row);
            layout.Controls.Add(labels[row].Item2, 1, row);
        }
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true };
        buttons.Controls.AddRange(new Control[] { _status, _tick, _capture, _history, _verify, _restore, _preview });
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 40));
        layout.Controls.Add(buttons, 1, labels.Length);
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        layout.Controls.Add(_output, 0, labels.Length + 1);
        layout.SetColumnSpan(_output, 2);
        Controls.Add(layout);
        _status.Click += async (_, _) => await SendAsync("status");
        _tick.Click += async (_, _) => await SendAsync("tick");
        _capture.Click += async (_, _) => await SendAsync("capture");
        _history.Click += async (_, _) => await SendAsync("history");
        _verify.Click += async (_, _) => await SendAsync("verify");
        _restore.Click += async (_, _) => await SendAsync("restore");
        _preview.Click += async (_, _) => await SendAsync("preview");
        FormClosing += (_, _) => StopAgent();
    }

    private bool StartAgent()
    {
        if (_pipe is { IsConnected: true }) return true;
        StopAgent();
        if (!File.Exists(_core.Text) || string.IsNullOrWhiteSpace(_endpoint.Text)) return false;
        var start = new ProcessStartInfo(_core.Text)
        {
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
        };
        start.ArgumentList.Add("agent");
        start.ArgumentList.Add("--endpoint");
        start.ArgumentList.Add(_endpoint.Text);
        _agent = Process.Start(start);
        if (_agent is null) return false;
        _pipe = new NamedPipeClientStream(".", _endpoint.Text, PipeDirection.InOut, PipeOptions.Asynchronous);
        try
        {
            _pipe.Connect(5000);
        }
        catch
        {
            StopAgent();
            return false;
        }
        _pipeInput = new StreamWriter(_pipe, new UTF8Encoding(false)) { AutoFlush = true };
        _pipeOutput = new StreamReader(_pipe, new UTF8Encoding(false));
        return true;
    }

    private void StopAgent()
    {
        try { _pipeInput?.Dispose(); } catch { }
        try { _pipeOutput?.Dispose(); } catch { }
        try { _pipe?.Dispose(); } catch { }
        try { _agent?.Kill(entireProcessTree: true); } catch { }
        _agent?.Dispose();
        _agent = null;
        _pipe = null;
        _pipeInput = null;
        _pipeOutput = null;
    }

    private string[] RepositoryArgs()
    {
        var args = new List<string>
        {
            "--repository", Path.GetFullPath(_repository.Text),
            "--backend", Path.GetFullPath(_backend.Text),
        };
        if (!string.IsNullOrWhiteSpace(_password.Text))
        {
            args.Add("--password-file");
            args.Add(Path.GetFullPath(_password.Text));
        }
        return args.ToArray();
    }

    private string[] BuildArguments(string operation) => operation switch
    {
        "status" => new[] { "schedule-status", "--config", Path.GetFullPath(_config.Text), "--state", Path.GetFullPath(_state.Text) },
        "tick" => new[] { "schedule-tick", "--config", Path.GetFullPath(_config.Text), "--state", Path.GetFullPath(_state.Text) },
        "capture" => new[] { "capture" }.Concat(RepositoryArgs()).Concat(new[] { "--source", Path.GetFullPath(_source.Text) }).ToArray(),
        "history" => new[] { "history" }.Concat(RepositoryArgs()).ToArray(),
        "verify" => new[] { "verify" }.Concat(RepositoryArgs())
            .Concat(string.IsNullOrWhiteSpace(_snapshot.Text) ? Array.Empty<string>() : new[] { "--snapshot", _snapshot.Text.Trim() })
            .ToArray(),
        "restore" => new[] { "restore" }.Concat(RepositoryArgs()).Concat(new[]
        {
            "--snapshot", _snapshot.Text.Trim(),
            "--destination", Path.GetFullPath(_destination.Text),
        }).ToArray(),
        "preview" => new[] { "retention", "preview" }.Concat(RepositoryArgs()).Concat(new[]
        {
            "--policy", Path.GetFullPath(_policy.Text),
            "--state", Path.GetFullPath(_state.Text),
        }).ToArray(),
        _ => throw new ArgumentOutOfRangeException(nameof(operation)),
    };

    private async Task SendAsync(string operation)
    {
        if (!StartAgent())
        {
            _output.Text = "Core executable or agent endpoint is unavailable.\r\n";
            return;
        }
        if (_pipeInput is null || _pipeOutput is null) return;
        foreach (var button in new[] { _status, _tick, _capture, _history, _verify, _restore, _preview })
            button.Enabled = false;
        try
        {
            var id = Interlocked.Increment(ref _requestId).ToString();
            var request = JsonSerializer.Serialize(new
            {
                schema_version = 1,
                request_id = id,
                argv = BuildArguments(operation),
            });
            await _pipeInput.WriteLineAsync(request);
            var response = await _pipeOutput.ReadLineAsync();
            _output.Text = response is null ? "Agent closed without a response." : Pretty(response);
        }
        catch (Exception error)
        {
            _output.Text = error.ToString();
            StopAgent();
        }
        finally
        {
            foreach (var button in new[] { _status, _tick, _capture, _history, _verify, _restore, _preview })
                button.Enabled = true;
        }
    }

    private static string Pretty(string json)
    {
        try
        {
            using var document = JsonDocument.Parse(json);
            return JsonSerializer.Serialize(document, new JsonSerializerOptions { WriteIndented = true });
        }
        catch
        {
            return json;
        }
    }
}
