using System.Diagnostics;
using System.Text.Json;

namespace PeerVault.Desktop;

public sealed class Form1 : Form
{
    private readonly TextBox _core = new() { Text = "peervault.exe", Width = 520 };
    private readonly TextBox _config = new() { Width = 520 };
    private readonly TextBox _state = new() { Width = 520 };
    private readonly RichTextBox _output = new() { ReadOnly = true, Dock = DockStyle.Fill, Font = new Font("Consolas", 9) };
    private readonly Button _status = new() { Text = "Status", Width = 100 };
    private readonly Button _tick = new() { Text = "Run due jobs", Width = 100 };
    private Process? _agent;
    private StreamWriter? _agentInput;
    private StreamReader? _agentOutput;
    private long _requestId;

    public Form1()
    {
        Text = "PeerVault scheduler (experimental)";
        Width = 900;
        Height = 640;
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, RowCount = 6, Padding = new Padding(8) };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 90));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 32));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 32));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 32));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 40));
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        layout.Controls.Add(new Label { Text = "Core", AutoSize = true }, 0, 0);
        layout.Controls.Add(_core, 1, 0);
        layout.Controls.Add(new Label { Text = "Jobs config", AutoSize = true }, 0, 1);
        layout.Controls.Add(_config, 1, 1);
        layout.Controls.Add(new Label { Text = "Scheduler state", AutoSize = true }, 0, 2);
        layout.Controls.Add(_state, 1, 2);
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Fill };
        buttons.Controls.Add(_status);
        buttons.Controls.Add(_tick);
        layout.Controls.Add(buttons, 1, 3);
        layout.Controls.Add(_output, 1, 4);
        Controls.Add(layout);
        _status.Click += async (_, _) => await SendAsync("status");
        _tick.Click += async (_, _) => await SendAsync("tick");
        FormClosing += (_, _) => StopAgent();
    }

    private bool StartAgent()
    {
        if (_agent is { HasExited: false }) return true;
        StopAgent();
        if (!File.Exists(_core.Text) || !File.Exists(_config.Text)) return false;
        var start = new ProcessStartInfo(_core.Text)
        {
            UseShellExecute = false,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
        };
        start.ArgumentList.Add("agent");
        start.ArgumentList.Add("--config");
        start.ArgumentList.Add(Path.GetFullPath(_config.Text));
        start.ArgumentList.Add("--state");
        start.ArgumentList.Add(Path.GetFullPath(_state.Text));
        _agent = Process.Start(start);
        if (_agent is null) return false;
        _agentInput = _agent.StandardInput;
        _agentOutput = _agent.StandardOutput;
        return true;
    }

    private void StopAgent()
    {
        try { _agent?.Kill(entireProcessTree: true); } catch { }
        _agent?.Dispose();
        _agent = null;
        _agentInput = null;
        _agentOutput = null;
    }

    private async Task SendAsync(string operation)
    {
        if (!StartAgent())
        {
            _output.Text = "Core or configuration path does not exist.\r\n";
            return;
        }
        if (_agentInput is null || _agentOutput is null) return;
        _status.Enabled = false;
        _tick.Enabled = false;
        try
        {
            var id = Interlocked.Increment(ref _requestId);
            var request = JsonSerializer.Serialize(new { schema_version = 1, id, op = operation });
            await _agentInput.WriteLineAsync(request);
            await _agentInput.FlushAsync();
            var response = await _agentOutput.ReadLineAsync();
            _output.Text = response is null ? "Agent closed without a response." : Pretty(response);
        }
        catch (Exception error)
        {
            _output.Text = error.ToString();
            StopAgent();
        }
        finally
        {
            _status.Enabled = true;
            _tick.Enabled = true;
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
