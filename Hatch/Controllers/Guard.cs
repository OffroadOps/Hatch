using System.Diagnostics;
using System.Text;
using Microsoft.VisualStudio.Threading;
using Hatch.Enums;
using Hatch.Models;
using Hatch.Utils;

namespace Hatch.Controllers;

public abstract class Guard
{
    /// <summary>进程启动超时总时长（毫秒）：MaxPollCount × PollIntervalMs = 50秒</summary>
    private const int ProcessStartMaxPollCount = 1000;
    /// <summary>每次轮询间隔（毫秒）</summary>
    private const int ProcessStartPollIntervalMs = 50;

    private FileStream? _logFileStream;
    private StreamWriter? _logStreamWriter;

    /// <param name="mainFile">Application path relative to Hatch\bin.</param>
    /// <param name="redirectOutput"></param>
    /// <param name="encoding">application output encode</param>
    protected Guard(string mainFile, bool redirectOutput = true, Encoding? encoding = null)
    {
        RedirectOutput = redirectOutput;

        var fileName = Path.GetFullPath($"bin\\{mainFile}");

        if (!File.Exists(fileName))
            throw new MessageException(i18N.Translate($"bin\\{mainFile} file not found!"));

        Instance = new Process
        {
            StartInfo =
            {
                FileName = fileName,
                WorkingDirectory = $"{Global.HatchDir}\\bin",
                CreateNoWindow = true,
                UseShellExecute = !RedirectOutput,
                RedirectStandardOutput = RedirectOutput,
                StandardOutputEncoding = RedirectOutput ? encoding : null,
                RedirectStandardError = RedirectOutput,
                StandardErrorEncoding = RedirectOutput ? encoding : null,
                WindowStyle = ProcessWindowStyle.Hidden
            }
        };
    }

    protected string LogPath => Path.Combine(Global.HatchDir, $"logging\\{Name}.log");

    protected virtual IEnumerable<string> StartedKeywords { get; } = new List<string>();

    protected virtual IEnumerable<string> FailedKeywords { get; } = new List<string>();

    public abstract string Name { get; }

    private State State { get; set; } = State.Waiting;

    private bool RedirectOutput { get; }

    public Process Instance { get; }

    protected async Task StartGuardAsync(string argument, ProcessPriorityClass priority = ProcessPriorityClass.Normal)
    {
        State = State.Starting;

        _logFileStream = new FileStream(LogPath, FileMode.Create, FileAccess.Write, FileShare.Read, 4096, true);
        _logStreamWriter = new StreamWriter(_logFileStream) { AutoFlush = true };

        Instance.StartInfo.Arguments = argument;
        Instance.Start();
        Global.Job.AddProcess(Instance);

        if (priority != ProcessPriorityClass.Normal)
            Instance.PriorityClass = priority;

        if (RedirectOutput)
        {
            ReadOutputAsync(Instance.StandardOutput).ContinueWith(t =>
            {
                if (t.IsFaulted && t.Exception != null)
                    Log.Error(t.Exception, "Failed to read stdout from {ProcessName}", Instance.ProcessName);
            });
            ReadOutputAsync(Instance.StandardError).ContinueWith(t =>
            {
                if (t.IsFaulted && t.Exception != null)
                    Log.Error(t.Exception, "Failed to read stderr from {ProcessName}", Instance.ProcessName);
            });

            if (!StartedKeywords.Any())
            {
                // Skip, No started keyword
                State = State.Started;
                return;
            }

            // wait ReadOutput change State
            for (var i = 0; i < ProcessStartMaxPollCount; i++)
            {
                await Task.Delay(ProcessStartPollIntervalMs);
                switch (State)
                {
                    case State.Started:
                        OnStarted();
                        return;
                    case State.Stopped:
                        await StopGuardAsync();
                        OnStartFailed();
                        throw new MessageException($"{Name} 控制器启动失败");
                }
            }

            await StopGuardAsync();
            throw new MessageException($"{Name} 控制器启动超时");
        }
    }

    private async Task ReadOutputAsync(TextReader reader)
    {
        string? line;
        while ((line = await reader.ReadLineAsync()) != null)
        {
            await _logStreamWriter!.WriteLineAsync(line);
            OnReadNewLine(line);

            if (State == State.Starting)
            {
                if (StartedKeywords.Any(s => line.Contains(s)))
                    State = State.Started;
                else if (FailedKeywords.Any(s => line.Contains(s)))
                {
                    OnStartFailed();
                    State = State.Stopped;
                }
            }
        }
    }

    public virtual Task StopAsync()
    {
        return StopGuardAsync();
    }

    protected async Task StopGuardAsync()
    {
        try
        {
            if (Instance is { HasExited: false })
            {
                try
                {
                    Instance.Kill();
                }
                catch (Exception killEx)
                {
                    Log.Warning(killEx, "Failed to kill {Name}, attempting to wait for exit", Instance.ProcessName);
                }

                try
                {
                    await Instance.WaitForExitAsync();
                }
                catch (Exception waitEx)
                {
                    Log.Warning(waitEx, "Failed to wait for {Name} exit", Instance.ProcessName);
                }
            }
        }
        catch (Exception e)
        {
            Log.Error(e, "Stop {Name} failed", Instance.ProcessName);
        }
        finally
        {
            if (_logStreamWriter != null)
                await _logStreamWriter.DisposeAsync();

            if (_logFileStream != null)
                await _logFileStream.DisposeAsync();

            Instance.Dispose();

            State = State.Stopped;
        }
    }

    protected virtual void OnStarted()
    {
    }

    protected virtual void OnReadNewLine(string line)
    {
    }

    protected virtual void OnStartFailed()
    {
        Utils.Utils.Open(LogPath);
    }
}
