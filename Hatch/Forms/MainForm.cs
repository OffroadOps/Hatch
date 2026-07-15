using System.ComponentModel;
using System.Diagnostics;
using System.Diagnostics.CodeAnalysis;
using System.Net;
using System.Net.Http;
using System.Net.Sockets;
using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.UI.WindowsAndMessaging;
using Microsoft.VisualStudio.Threading;
using Microsoft.Win32;
using Hatch.Controllers;
using Hatch.Enums;
using Hatch.Forms.ModeForms;
using Hatch.Interfaces;
using Hatch.Models;
using Hatch.Models.Modes;
using Hatch.Properties;
using Hatch.Services;
using Hatch.Utils;

namespace Hatch.Forms;

[Fody.ConfigureAwait(true)]
public partial class MainForm : Form
{
    /// <summary>延迟颜色阈值：低于此值显示绿色（良好）</summary>
    private const int LatencyGoodThresholdMs = 80;
    /// <summary>延迟颜色阈值：低于此值显示橙色（一般），高于显示红色（差）</summary>
    private const int LatencyFairThresholdMs = 200;

    #region Start

    private readonly Dictionary<string, object> _mainFormText = new();

    private bool _textRecorded;

    public MainForm()
    {
        InitializeComponent();
        NotifyIcon.Icon = Icon = Resources.icon;

        AddAddServerToolStripMenuItems();

        #region i18N Translations

        if (Flags.NoSupport)
            _mainFormText.Add(Name, new[] { "{0} ({1})", "Hatch", "No Support" });

        _mainFormText.Add(UninstallServiceToolStripMenuItem.Name, new[] { "Uninstall {0}", "NF Service" });

        #endregion

        // 监听电源事件
        SystemEvents.PowerModeChanged += SystemEvents_PowerModeChanged;

        // 更新检查事件
        UpdateChecker.NewVersionFound += UpdateChecker_NewVersionFound;
        UpdateChecker.NewVersionNotFound += UpdateChecker_NewVersionNotFound;
        UpdateChecker.NewVersionFoundFailed += UpdateChecker_NewVersionFoundFailed;
    }

    private void AddAddServerToolStripMenuItems()
    {
        foreach (var serversUtil in ServerHelper.ServerUtilDictionary.Values.OrderBy(i => i.Priority).Where(i => !string.IsNullOrEmpty(i.FullName)))
        {
            var fullName = serversUtil.FullName;
            var control = new ToolStripMenuItem
            {
                Name = $"Add{fullName}ServerToolStripMenuItem",
                Size = new Size(259, 22),
                Text = i18N.TranslateFormat("Add [{0}] Server", fullName),
                Tag = serversUtil
            };

            _mainFormText.Add(control.Name, new[] { "Add [{0}] Server", fullName });
            control.Click += AddServerToolStripMenuItem_Click;
            ServerToolStripMenuItem.DropDownItems.Add(control);
        }
    }

    private void MainForm_Load(object sender, EventArgs e)
    {
        // 计算 ComboBox绘制 目标宽度
        RecordSize();

        LoadServers();
        SelectLastServer();
        DelayTestHelper.UpdateTick(true);

        ModeService.Instance.Load();

        // 加载翻译
        TranslateControls();

        // 隐藏 ConnectivityStatusLabel
        ConnectivityStatusVisible(false);

        // 加载快速配置
        LoadProfiles();

        // 检查更新
        if (Global.Settings.CheckUpdateWhenOpened)
        {
            CheckUpdateAsync().ContinueWith(t =>
            {
                if (t.IsFaulted && t.Exception != null)
                    Log.Error(t.Exception, "Check update failed on startup");
            });
        }

        // 检查订阅更新
        if (Global.Settings.UpdateServersWhenOpened)
        {
            UpdateServersFromSubscriptionAsync().ContinueWith(t =>
            {
                if (t.IsFaulted && t.Exception != null)
                    Log.Error(t.Exception, "Update servers from subscription failed on startup");
            });
        }

        // 打开软件时启动加速，产生开始按钮点击事件
        if (Global.Settings.StartWhenOpened)
            ControlButton.PerformClick();

        Program.SingleInstance.StartListenServer();
    }

    private void RecordSize()
    {
        _numberBoxWidth = ServerComboBox.Width / 10;
        _numberBoxX = (int)(_numberBoxWidth * 8.5);  // 从90%改为85%，给延迟文字留更多空间
        _numberBoxWrap = _numberBoxWidth / 30;

        _configurationGroupBoxHeight = ConfigurationGroupBox.Height;
        _profileConfigurationHeight = ConfigurationGroupBox.Controls[0].Height / 3; // 因为 AutoSize, 所以得到的是Controls的总高度
        _profileGroupBoxPaddingHeight = ProfileGroupBox.Height - ProfileTable.Height;
        _profileTableHeight = ProfileTable.Height;
    }

    private void TranslateControls()
    {
        #region Record English

        if (!_textRecorded)
        {
            void RecordText(Component component)
            {
                try
                {
                    switch (component)
                    {
                        case TextBoxBase:
                        case ListControl:
                            break;
                        case Control c:
                            _mainFormText.Add(c.Name, c.Text);
                            break;
                        case ToolStripItem c:
                            _mainFormText.Add(c.Name, c.Text);
                            break;
                    }
                }
                catch (ArgumentException)
                {
                    // ignored
                }
            }

            Utils.Utils.ComponentIterator(this, RecordText);
            Utils.Utils.ComponentIterator(NotifyMenu, RecordText);
            _textRecorded = true;
        }

        #endregion

        #region Translate

        void TranslateText(Component component)
        {
            switch (component)
            {
                case TextBoxBase:
                case ListControl:
                    break;
                case Control c:
                    if (_mainFormText.ContainsKey(c.Name))
                        c.Text = ControlText(c.Name);

                    break;
                case ToolStripItem c:
                    if (_mainFormText.ContainsKey(c.Name))
                        c.Text = ControlText(c.Name);

                    break;
            }

            string ControlText(string name)
            {
                var value = _mainFormText[name];
                if (value.Equals(string.Empty))
                    return string.Empty;

                if (value is object[] values)
                    return i18N.TranslateFormat((string)values.First(), values.Skip(1).ToArray());

                return i18N.Translate(value);
            }
        }

        Utils.Utils.ComponentIterator(this, TranslateText);
        Utils.Utils.ComponentIterator(NotifyMenu, TranslateText);

        #endregion

        UsedBandwidthLabel.Text = $@"{i18N.Translate("Used", ": ")}0 KB";
        State = State;
    }

    #endregion

    #region Controls

    #region MenuStrip

    #region Server

    private void ImportServersFromClipboardToolStripMenuItem_Click(object sender, EventArgs e)
    {
        ImportServersFromClipboardAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Import servers from clipboard failed");
        });
    }

    private async Task ImportServersFromClipboardAsync()
    {
        var texts = Clipboard.GetText();
        if (string.IsNullOrWhiteSpace(texts))
            return;

        var servers = ShareLink.ParseText(texts);
        foreach (var server in servers)
            server.Group = Constants.DefaultGroup;

        Global.Settings.Server.AddRange(servers);
        NotifyTip(i18N.TranslateFormat("Import {0} server(s) form Clipboard", servers.Count));

        LoadServers();
        await Configuration.SaveAsync();
    }

    private void AddServerToolStripMenuItem_Click([NotNull] object? sender, EventArgs? e)
    {
        AddServerAsync(sender, e).ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Add server failed");
        });
    }

    private async Task AddServerAsync([NotNull] object? sender, EventArgs? e)
    {
        if (sender == null)
            throw new ArgumentNullException(nameof(sender));

        var util = (IServerUtil)((ToolStripMenuItem)sender).Tag;

        Hide();
        util.Create();

        LoadServers();
        await Configuration.SaveAsync();
        Show();
    }

    #endregion

    #region Mode

    private void CreateProcessModeToolStripButton_Click(object sender, EventArgs e)
    {
        Hide();
        new ProcessForm().ShowDialog();
        Show();
    }

    private void createRouteTableModeToolStripMenuItem_Click(object sender, EventArgs e)
    {
        Hide();
        new RouteForm().ShowDialog();
        Show();
    }

    private void ReloadModesToolStripMenuItem_Click(object sender, EventArgs e)
    {
        Enabled = false;
        try
        {
            ModeService.Instance.Load();
        }
        finally
        {
            Enabled = true;
        }
    }

    #endregion

    #region Subscription

    private void ManageSubscriptionLinksToolStripMenuItem_Click(object sender, EventArgs e)
    {
        Hide();
        new SubscriptionForm().ShowDialog();
        LoadServers();
        Show();
    }

    private void UpdateServersFromSubscriptionLinksToolStripMenuItem_Click(object sender, EventArgs e)
    {
        UpdateServersFromSubscriptionAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Update servers from subscription failed");
        });
    }

    private async Task UpdateServersFromSubscriptionAsync()
    {
        void DisableItems(bool v)
        {
            MenuStrip.Enabled = ConfigurationGroupBox.Enabled = ProfileGroupBox.Enabled = ControlButton.Enabled = v;
        }

        if (Global.Settings.Subscription.Count <= 0)
        {
            MessageBoxX.Show(i18N.Translate("No subscription link"));
            return;
        }

        StatusText(i18N.Translate("Updating servers"));
        DisableItems(false);

        try
        {
            await SubscriptionUtil.UpdateServersAsync();

            LoadServers();
            await Configuration.SaveAsync();
            StatusText(i18N.Translate("Servers updated"));
        }
        catch (Exception e)
        {
            NotifyTip(i18N.Translate("Unhandled update servers error") + "\n" + e.Message, info: false);
            Log.Error(e, "Unhandled Update servers error");
        }
        finally
        {
            DisableItems(true);
        }
    }

    #endregion

    #region Options

    private void OpenDirectoryToolStripMenuItem_Click(object sender, EventArgs e)
    {
        Utils.Utils.Open(".\\");
    }

    private void CleanDNSCacheToolStripMenuItem_Click(object sender, EventArgs e)
    {
        CleanDNSCacheAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Clean DNS cache failed");
        });
    }

    private async Task CleanDNSCacheAsync()
    {
        try
        {
            await Task.Run(() =>
            {
                NativeMethods.RefreshDNSCache();
                DnsUtils.ClearCache();
            });

            NotifyTip(i18N.Translate("DNS cache cleanup succeeded"));
        }
        catch (Exception e)
        {
            Log.Warning(e, "DNS cache cleanup failed");
            NotifyTip(i18N.Translate("DNS cache cleanup failed"), info: false);
        }
        finally
        {
            StatusText();
        }
    }

    private void UninstallServiceToolStripMenuItem_Click(object sender, EventArgs e)
    {
        UninstallServiceAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Uninstall service failed");
        });
    }

    private async Task UninstallServiceAsync()
    {
        Enabled = false;
        StatusText(i18N.TranslateFormat("Uninstalling {0}", "NF Service"));
        try
        {
            var task = Task.Run(NFController.UninstallDriver);

            if (await task)
                NotifyTip(i18N.TranslateFormat("{0} has been uninstalled", "NF Service"));
        }
        catch (Exception e)
        {
            Log.Error(e, "Failed to uninstall NF Service");
            NotifyTip(i18N.Translate("Failed to uninstall service"), info: false);
        }
        finally
        {
            StatusText();
            Enabled = true;
        }
    }

    private void RemoveHatchFirewallRulesToolStripMenuItem_Click(object sender, EventArgs e)
    {
        Firewall.RemoveHatchFwRules();
    }

    private void ShowHideConsoleToolStripMenuItem_Click(object sender, EventArgs e)
    {
        var windowStyles = (WINDOW_STYLE)PInvoke.GetWindowLong(new HWND(Program.ConsoleHwnd), WINDOW_LONG_PTR_INDEX.GWL_STYLE);
        var visible = windowStyles.HasFlag(WINDOW_STYLE.WS_VISIBLE);
        PInvoke.ShowWindow(Program.ConsoleHwnd, visible ? SHOW_WINDOW_CMD.SW_HIDE : SHOW_WINDOW_CMD.SW_SHOWNOACTIVATE);
    }

    #endregion

    /// <summary>
    ///     菜单栏强制退出
    /// </summary>
    private void ForceExitToolStripMenuItem_Click(object sender, EventArgs e)
    {
        Exit(true);
    }

    /// <summary>
    ///     关于菜单
    /// </summary>
    private void AboutToolStripMenuItem_Click(object sender, EventArgs e)
    {
        var aboutForm = new AboutForm();
        aboutForm.ShowDialog(this);
    }

    #endregion

    #region ControlButton

    private void ControlButton_Click(object? sender, EventArgs? e)
    {
        ControlButtonClickAsync(sender, e).ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
            {
                Log.Error(t.Exception, "Control button click failed");
                BeginInvoke(() =>
                {
                    State = State.Stopped;
                    MessageBoxX.Show(t.Exception.GetBaseException().Message, LogLevel.ERROR);
                });
            }
        });
    }

    private async Task ControlButtonClickAsync(object? sender, EventArgs? e)
    {
        if (!IsWaiting())
        {
            await StopCoreAsync();
            return;
        }

        Configuration.SaveAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Save configuration failed");
        });

        // 服务器、模式 需选择
        if (ServerComboBox.SelectedItem is not Server server)
        {
            MessageBoxX.Show(i18N.Translate("Please select a server first"));
            return;
        }

        if (ModeComboBox.SelectedItem is not Mode mode)
        {
            MessageBoxX.Show(i18N.Translate("Please select a mode first"));
            return;
        }

        State = State.Starting;

        try
        {
            await MainController.StartAsync(server, mode);
        }
        catch (Exception exception)
        {
            State = State.Stopped;
            StatusText(i18N.Translate("Start failed"));
            MessageBoxX.Show(exception.Message, LogLevel.ERROR);
            return;
        }

        State = State.Started;

        Task.Run(Bandwidth.NetTraffic).ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "NetTraffic monitoring failed");
        });

        DiscoveryNatTypeAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "NAT type discovery failed");
        });

        HttpConnectAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "HTTP connect test failed");
        });

        if (Global.Settings.MinimizeWhenStarted)
            Minimize();

        // 自动检测延迟
        async Task StartedPingAsync()
        {
            while (State == State.Started)
            {
                if (Global.Settings.StartedPingInterval >= 0)
                {
                    await server.PingAsync();
                    ServerComboBox.Refresh();

                    await Task.Delay(Global.Settings.StartedPingInterval * 1000);
                }
                else
                {
                    await Task.Delay(5000);
                }
            }
        }

        StartedPingAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Started ping test failed");
        });
    }

    #endregion

    #region SettingsButton

    private void SettingsButton_Click(object sender, EventArgs e)
    {
        var oldSettings = Global.Settings.ShallowCopy();

        Hide();
        new SettingForm().ShowDialog();

        if (oldSettings.Language != Global.Settings.Language)
        {
            i18N.Load(Global.Settings.Language);
            TranslateControls();
            LoadModes();
            LoadProfiles();
        }

        if (oldSettings.DetectionTick != Global.Settings.DetectionTick)
            DelayTestHelper.UpdateTick(true);

        if (oldSettings.ProfileCount != Global.Settings.ProfileCount)
            LoadProfiles();

        Show();
    }

    #endregion

    #region Server

    private void LoadServers()
    {
        ServerComboBox.Items.Clear();
        ServerComboBox.Items.AddRange(Global.Settings.Server.Cast<object>().ToArray());
        SelectLastServer();
    }

    private void SelectLastServer()
    {
        // 如果值合法，选中该位置
        if (Global.Settings.ServerComboBoxSelectedIndex >= 0 && Global.Settings.ServerComboBoxSelectedIndex < ServerComboBox.Items.Count)
            ServerComboBox.SelectedIndex = Global.Settings.ServerComboBoxSelectedIndex;
        // 如果值非法，且当前 ServerComboBox 中有元素，选择第一个位置
        else if (ServerComboBox.Items.Count > 0)
            ServerComboBox.SelectedIndex = 0;

        // 如果当前 ServerComboBox 中没元素，不做处理
    }

    private void ServerComboBox_SelectionChangeCommitted(object sender, EventArgs o)
    {
        Global.Settings.ServerComboBoxSelectedIndex = ServerComboBox.SelectedIndex;
    }

    private void EditServerPictureBox_Click(object sender, EventArgs e)
    {
        EditServerAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Edit server failed");
        });
    }

    private async Task EditServerAsync()
    {
        // 当前ServerComboBox中至少有一项
        if (!(ServerComboBox.SelectedItem is Server server))
        {
            MessageBoxX.Show(i18N.Translate("Please select a server first"));
            return;
        }

        Hide();
        ServerHelper.GetUtilByTypeName(server.Type).Edit(server);
        LoadServers();
        await Configuration.SaveAsync();
        Show();
    }

    private void SpeedPictureBox_Click(object sender, EventArgs e)
    {
        SpeedTestAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Speed test failed");
        });
    }

    private async Task SpeedTestAsync()
    {
        void Enable()
        {
            ServerComboBox.Refresh();
            Enabled = true;
            StatusText();
        }

        // 检查是否已连接（IsWaiting返回true表示未运行，false表示正在运行）
        if (IsWaiting())
        {
            MessageBoxX.Show(i18N.Translate("Please start connection before testing IP and region info"), LogLevel.WARNING, i18N.Translate("Tip"));
            return;
        }

        Enabled = false;
        StatusText(i18N.Translate("Testing"));

        var server = ServerComboBox.SelectedItem as Server;
        if (server == null)
        {
            MessageBoxX.Show(i18N.Translate("Please select a server first"), LogLevel.WARNING, i18N.Translate("Tip"));
            Enable();
            return;
        }

        // 通过 Hatch 的 SOCKS5 代理测试延迟（而非直接 ping 服务器）
        string delayText;
        try
        {
            var socks5Server = MainController.Socks5Server;
            if (socks5Server != null)
            {
                using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(10));
                var proxyDelay = await MainController.HttpConnectAsync(cts.Token);
                delayText = proxyDelay != null ? $"{proxyDelay}ms" : i18N.Translate("Connection failed");
            }
            else
            {
                delayText = i18N.Translate("Connection failed");
            }
        }
        catch
        {
            delayText = i18N.Translate("Connection failed");
        }

        // 获取IP和地区信息（通过 Hatch 的 SOCKS5 代理）
        string ipInfo = "";
        try
        {
            if (State != State.Started)
            {
                ipInfo = "\n\n" + i18N.Translate("Start Hatch to get IP info");
            }
            else
            {
                var socks5Server = MainController.Socks5Server;
                if (socks5Server == null)
                {
                    ipInfo = "\n\n" + i18N.Translate("Unable to get proxy info");
                }
                else
                {
                    // 使用 HttpClient + SOCKS5 代理，DNS 也走代理（等同于 socks5h）
                    try
                    {
                        var proxy = new WebProxy($"socks5://127.0.0.1:{socks5Server.Port}");
                        using var handler = new SocketsHttpHandler
                        {
                            Proxy = proxy,
                            UseProxy = true,
                            ConnectTimeout = TimeSpan.FromSeconds(10)
                        };
                        using var httpClient = new HttpClient(handler) { Timeout = TimeSpan.FromSeconds(15) };

                        var response = await httpClient.GetStringAsync("https://api.ip.sb/geoip");

                        if (!string.IsNullOrWhiteSpace(response))
                        {
                            var json = System.Text.Json.JsonDocument.Parse(response);
                            var ip = json.RootElement.GetProperty("ip").GetString();
                            var country = json.RootElement.GetProperty("country").GetString();
                            var city = json.RootElement.TryGetProperty("city", out var cityProp) ? cityProp.GetString() : "";

                            ipInfo = $"\n\nIP: {ip}\n{i18N.Translate("Region")}: {country}" + (string.IsNullOrEmpty(city) ? "" : $" {city}");
                        }
                        else
                        {
                            throw new Exception("Empty response");
                        }
                    }
                    catch
                    {
                        // 备用 API
                        try
                        {
                            var proxy = new WebProxy($"socks5://127.0.0.1:{socks5Server.Port}");
                            using var handler = new SocketsHttpHandler
                            {
                                Proxy = proxy,
                                UseProxy = true,
                                ConnectTimeout = TimeSpan.FromSeconds(10)
                            };
                            using var httpClient = new HttpClient(handler) { Timeout = TimeSpan.FromSeconds(15) };

                            var response = await httpClient.GetStringAsync("https://myip.ipip.net");

                            if (!string.IsNullOrWhiteSpace(response))
                            {
                                ipInfo = $"\n\n{i18N.Translate("IP Info")}: {response.Trim()}";
                            }
                            else
                            {
                                ipInfo = "\n\n" + i18N.Translate("Unable to get IP info");
                            }
                        }
                        catch
                        {
                            ipInfo = "\n\n" + i18N.Translate("Unable to get IP info");
                        }
                    }
                }
            }
        }
        catch (Exception ex)
        {
            ipInfo = $"\n\n{i18N.Translate("Failed to get IP info")}: {ex.Message}";
        }

        var message = $"{i18N.Translate("Server")}: {server.Remark}\n{i18N.Translate("Delay")}: {delayText}{ipInfo}";
        MessageBoxX.Show(message, LogLevel.INFO, i18N.Translate("Speed Test Result"));

        Enable();
    }

    private void CopyLinkPictureBox_Click(object sender, EventArgs e)
    {
        // 当前ServerComboBox中至少有一项
        if (!(ServerComboBox.SelectedItem is Server server))
        {
            MessageBoxX.Show(i18N.Translate("Please select a server first"));
            return;
        }

        try
        {
            //听说巨硬BUG经常会炸，所以Catch一下 :D
            string text;
            if (ModifierKeys == Keys.Control)
                text = ShareLink.GetHatchLink(server);
            else
                text = ShareLink.GetShareLink(server);

            Clipboard.SetText(text);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to copy clipboard text");
        }
    }

    private void DeleteServerPictureBox_Click(object sender, EventArgs e)
    {
        // 当前 ServerComboBox 中至少有一项
        if (!(ServerComboBox.SelectedItem is Server server))
        {
            MessageBoxX.Show(i18N.Translate("Please select a server first"));
            return;
        }

        Global.Settings.Server.Remove(server);
        LoadServers();
    }

    #endregion

    #region Mode

    public void LoadModes()
    {
        if (InvokeRequired)
        {
            Invoke(LoadModes);
            return;
        }

        ModeComboBox.Items.Clear();
        ModeComboBox.Items.AddRange(Global.Modes.Cast<object>().ToArray());
        ModeComboBox.Tag = null;
        SelectLastMode();
    }

    private void SelectLastMode()
    {
        // 如果值合法，选中该位置
        if (Global.Settings.ModeComboBoxSelectedIndex >= 0 && Global.Settings.ModeComboBoxSelectedIndex < ModeComboBox.Items.Count)
            ModeComboBox.SelectedIndex = Global.Settings.ModeComboBoxSelectedIndex;
        // 如果值非法，且当前 ModeComboBox 中有元素，选择第一个位置
        else if (ModeComboBox.Items.Count > 0)
            ModeComboBox.SelectedIndex = 0;

        // 如果当前 ModeComboBox 中没元素，不做处理
    }

    private void ModeComboBox_SelectionChangeCommitted(object sender, EventArgs o)
    {
        try
        {
            Global.Settings.ModeComboBoxSelectedIndex = Global.Modes.IndexOf((Mode)ModeComboBox.SelectedItem);
        }
        catch
        {
            Global.Settings.ModeComboBoxSelectedIndex = 0;
        }
    }

    private void EditModePictureBox_Click(object sender, EventArgs e)
    {
        // 当前ModeComboBox中至少有一项
        if (ModeComboBox.SelectedIndex == -1)
        {
            MessageBoxX.Show(i18N.Translate("Please select a mode first"));
            return;
        }

        var mode = (Mode)ModeComboBox.SelectedItem;
        if (ModifierKeys == Keys.Control)
        {
            Utils.Utils.Open(mode.FullName);
            return;
        }

        switch (mode.Type)
        {
            case ModeType.ProcessMode:
                Hide();
                new ProcessForm(mode).ShowDialog();
                Show();
                break;
            case ModeType.TunMode:
                Hide();
                new RouteForm(mode).ShowDialog();
                Show();
                break;
            case ModeType.ShareMode:
            // throw new NotImplementedException();
            default:
                Utils.Utils.Open(mode.FullName);
                break;
        }
    }

    private void DeleteModePictureBox_Click(object sender, EventArgs e)
    {
        // 当前ModeComboBox中至少有一项
        if (ModeComboBox.Items.Count <= 0 || ModeComboBox.SelectedIndex == -1)
        {
            MessageBoxX.Show(i18N.Translate("Please select a mode first"));
            return;
        }

        ModeService.Delete((Mode)ModeComboBox.SelectedItem);
        SelectLastMode();
    }

    #endregion

    #region Profile

    private int _configurationGroupBoxHeight;
    private int _profileConfigurationHeight;
    private int _profileGroupBoxPaddingHeight;
    private int _profileTableHeight;

    private void LoadProfiles()
    {
        // Clear
        foreach (var button in ProfileTable.Controls)
            ((Button)button).Dispose();

        ProfileTable.Controls.Clear();
        ProfileTable.ColumnStyles.Clear();
        ProfileTable.RowStyles.Clear();

        var profileCount = Global.Settings.ProfileCount;
        if (profileCount == 0)
        {
            // Hide Profile GroupBox, Change window size
            configLayoutPanel.RowStyles[2].SizeType = SizeType.Percent;
            configLayoutPanel.RowStyles[2].Height = 0;
            ProfileGroupBox.Visible = false;

            ConfigurationGroupBox.Height = _configurationGroupBoxHeight - _profileConfigurationHeight;
        }
        else
        {
            // Load Profiles

            if (Global.Settings.ProfileTableColumnCount == 0)
                Global.Settings.ProfileTableColumnCount = 5;

            var columnCount = Global.Settings.ProfileTableColumnCount;

            ProfileTable.ColumnCount = profileCount >= columnCount ? columnCount : profileCount;
            ProfileTable.RowCount = (int)Math.Ceiling(profileCount / (float)columnCount);

            for (var i = 0; i < profileCount; ++i)
            {
                var profile = Global.Settings.Profiles.SingleOrDefault(p => p.Index == i);
                var b = new Button
                {
                    Dock = DockStyle.Fill,
                    Text = profile?.ProfileName ?? i18N.Translate("None"),
                    Tag = profile
                };

                b.Click += ProfileButton_Click;
                ProfileTable.Controls.Add(b, i % columnCount, i / columnCount);
            }

            // equal column - 修复：确保 RowCount 和 ColumnCount 大于 0
            if (ProfileTable.RowCount > 0)
            {
                for (var i = 0; i < ProfileTable.RowCount; i++)
                    ProfileTable.RowStyles.Add(new RowStyle(SizeType.Percent, 1));
            }

            if (ProfileTable.ColumnCount > 0)
            {
                for (var i = 0; i < ProfileTable.ColumnCount; i++)
                    ProfileTable.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 1));
            }

            configLayoutPanel.RowStyles[2].SizeType = SizeType.AutoSize;
            ProfileGroupBox.Visible = true;
            ProfileGroupBox.Height = ProfileTable.RowCount * _profileTableHeight + _profileGroupBoxPaddingHeight;
            ConfigurationGroupBox.Height = _configurationGroupBoxHeight;
        }
    }

    private void ProfileButton_Click([NotNull] object? sender, EventArgs? e)
    {
        ProfileButtonClickAsync(sender, e).ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Profile button click failed");
        });
    }

    private async Task ProfileButtonClickAsync([NotNull] object? sender, EventArgs? e)
    {
        if (sender == null)
            throw new InvalidOperationException();

        var button = (Button)sender;
        var profile = (Profile?)button.Tag;
        var index = ProfileTable.Controls.IndexOf(button);

        switch (ModifierKeys)
        {
            case Keys.Control:
                // Save Profile
                if (ServerComboBox.SelectedItem is not Server server)
                {
                    MessageBoxX.Show(i18N.Translate("Please select a server first"));
                    return;
                }

                if (ModeComboBox.SelectedItem is not Mode mode)
                {
                    MessageBoxX.Show(i18N.Translate("Please select a mode first"));
                    return;
                }

                var name = ProfileNameText.Text;

                Global.Settings.Profiles.RemoveAll(p => p.Index == index);
                profile = new Profile(server, mode, name, index);
                Global.Settings.Profiles.Add(profile);
                button.Tag = profile;
                button.Text = profile.ProfileName;

                ProfileNameText.Clear();
                return;
            case Keys.Shift:
                // Delete Profile
                if (profile == null)
                    return;

                Global.Settings.Profiles.Remove(profile);
                button.Tag = null;
                button.Text = i18N.Translate("None");
                return;
        }

        // Activate Profile

        if (profile == null)
        {
            MessageBoxX.Show(i18N.Translate("No saved profile here. Save a profile first by Ctrl+Click on the button"));
            return;
        }

        try
        {
            ProfileNameText.Text = profile.ProfileName;

            var server = ServerComboBox.Items.Cast<Server>().FirstOrDefault(s => s.Remark.Equals(profile.ServerRemark));
            var mode = ModeComboBox.Items.Cast<Mode>().FirstOrDefault(m => m.Remark.Any(s => s.Value.Equals(profile.ModeRemark)));

            if (server == null)
                throw new MessageException("Server not found.");

            if (mode == null)
                throw new MessageException("Mode not found.");

            // set active server and mode
            ServerComboBox.SelectedItem = server;
            ModeComboBox.SelectedItem = mode;
        }
        catch (MessageException exception)
        {
            MessageBoxX.Show(exception.Message, LogLevel.ERROR);
            return;
        }

        await StopAsync();
        ControlButton.PerformClick();
    }

    #endregion

    #region State

    private State _state = State.Waiting;

    /// <summary>
    ///     当前状态
    /// </summary>
    public State State
    {
        get => _state;
        private set
        {
            void StartDisableItems(bool enabled)
            {
                ServerComboBox.Enabled = ModeComboBox.Enabled = EditModePictureBox.Enabled =
                    EditServerPictureBox.Enabled = DeleteModePictureBox.Enabled = DeleteServerPictureBox.Enabled = enabled;

                // 启动需要禁用的控件
                ServerToolStripMenuItem.Enabled = ModeToolStripMenuItem.Enabled =
                    SubscriptionToolStripMenuItem.Enabled = UninstallServiceToolStripMenuItem.Enabled = enabled;
            }

            _state = value;

            DelayTestHelper.Enabled = IsWaiting(_state);

            StatusText();
            switch (value)
            {
                case State.Waiting:
                    ControlButton.Enabled = true;
                    ControlButton.Text = i18N.Translate("Start");

                    break;
                case State.Starting:
                    ControlButton.Enabled = false;
                    ControlButton.Text = "...";

                    ProfileGroupBox.Enabled = false;
                    StartDisableItems(false);
                    break;
                case State.Started:
                    ControlButton.Enabled = true;
                    ControlButton.Text = i18N.Translate("Stop");

                    ProfileGroupBox.Enabled = true;

                    break;
                case State.Stopping:
                    ControlButton.Enabled = false;
                    ControlButton.Text = "...";

                    ProfileGroupBox.Enabled = false;
                    BandwidthState(false);
                    ConnectivityStatusVisible(false);
                    break;
                case State.Stopped:
                    ControlButton.Enabled = true;
                    ControlButton.Text = i18N.Translate("Start");

                    LastUploadBandwidth = 0;
                    LastDownloadBandwidth = 0;
                    Bandwidth.Stop();

                    ProfileGroupBox.Enabled = true;
                    StartDisableItems(true);
                    break;
            }
        }
    }

    public async Task StopAsync()
    {
        if (IsWaiting())
            return;

        await StopCoreAsync();
    }

    private async Task StopCoreAsync()
    {
        State = State.Stopping;
        _discoveryNatCts?.Cancel();
        _httpConnectCts?.Cancel();
        await MainController.StopAsync();
        State = State.Stopped;
    }

    private bool IsWaiting() => IsWaiting(_state);

    private static bool IsWaiting(State state)
    {
        return state is State.Waiting or State.Stopped;
    }

    /// <summary>
    ///     更新状态栏文本
    /// </summary>
    /// <param name="text"></param>
    public void StatusText(string? text = null)
    {
        if (InvokeRequired)
        {
            BeginInvoke(() => StatusText(text));
            return;
        }

        text ??= i18N.Translate(StateExtension.GetStatusString(State));
        if (_state == State.Started)
            text += StatusPortInfoText.Value;

        StatusLabel.Text = i18N.Translate("Status", ": ") + text;
    }

    public void BandwidthState(bool state)
    {
        if (InvokeRequired)
        {
            BeginInvoke(() => BandwidthState(state));
            return;
        }

        if (IsWaiting())
            return;

        UsedBandwidthLabel.Visible /*= UploadSpeedLabel.Visible*/ = DownloadSpeedLabel.Visible = state;
    }

    private void UpdateNatTypeStatusLabelText(string? text, string? country = null)
    {
        if (!string.IsNullOrEmpty(text))
        {
            if (country == null)
                NatTypeStatusLabel.Text = $"NAT{i18N.Translate(": ")}{text} ";
            else
                NatTypeStatusLabel.Text = $"NAT{i18N.Translate(": ")}{text} [{country}]";

            UpdateNatTypeLight(int.TryParse(text, out var natType) ? natType : -1);
        }
        else
        {
            NatTypeStatusLabel.Text = $@"NAT{i18N.Translate(": ", "Test failed")}";
        }

        NatTypeStatusLabel.Visible = true;
    }

    private void ConnectivityStatusVisible(bool visible)
    {
        if (!visible)
            HttpStatusLabel.Text = NatTypeStatusLabel.Text = "";

        HttpStatusLabel.Visible = NatTypeStatusLabel.Visible = NatTypeStatusLightLabel.Visible = visible;
    }

    /// <summary>
    ///     更新 NAT指示灯颜色
    /// </summary>
    /// <param name="natType">NAT Type. keep default(-1) to Hide Light</param>
    private void UpdateNatTypeLight(int natType = -1)
    {
        if (natType > 0 && natType < 5)
        {
            NatTypeStatusLightLabel.Visible = Flags.IsWindows10Upper;
            var c = natType switch
            {
                1 => Color.LimeGreen,
                2 => Color.Yellow,
                3 => Color.Red,
                4 => Color.Black,
                _ => throw new ArgumentOutOfRangeException(nameof(natType), natType, null)
            };

            NatTypeStatusLightLabel.ForeColor = c;
        }
        else
        {
            NatTypeStatusLightLabel.Visible = false;
        }
    }

    private void TcpStatusLabel_Click(object sender, EventArgs e)
    {
        HttpConnectAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "HTTP connect test failed");
        });
    }

    private void NatTypeStatusLabel_Click(object sender, EventArgs e)
    {
        DiscoveryNatTypeAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "NAT type discovery failed");
        });
    }

    private CancellationTokenSource? _discoveryNatCts;

    private async Task DiscoveryNatTypeAsync()
    {
        NatTypeStatusLabel.Enabled = false;
        UpdateNatTypeStatusLabelText(i18N.Translate("Testing NAT Type"));

        _discoveryNatCts = new CancellationTokenSource();

        try
        {
            var res = await MainController.DiscoveryNatTypeAsync(_discoveryNatCts.Token);
            if (_discoveryNatCts.IsCancellationRequested)
                return;

            if (!string.IsNullOrEmpty(res.PublicEnd))
            {
                var country = await Utils.Utils.GetCityCodeAsync(res.PublicEnd);

                UpdateNatTypeStatusLabelText(res.Result, country);
                if (int.TryParse(res.Result, out var natType))
                    UpdateNatTypeLight(natType);
                else
                    UpdateNatTypeLight();
            }
            else
            {
                UpdateNatTypeStatusLabelText(res.Result ?? "Error");
                NatTypeStatusLightLabel.Visible = false;
            }
        }
        finally
        {
            _discoveryNatCts.Dispose();
            _discoveryNatCts = null;
            NatTypeStatusLabel.Enabled = true;
        }
    }

    private CancellationTokenSource? _httpConnectCts;

    private async Task HttpConnectAsync()
    {
        HttpStatusLabel.Enabled = false;

        _httpConnectCts = new CancellationTokenSource();

        try
        {
            var res = await MainController.HttpConnectAsync(_httpConnectCts.Token);
            if (_httpConnectCts.IsCancellationRequested)
                return;

            if (res != null)
                HttpStatusLabel.Text = $"HTTP{i18N.Translate(": ")}{res}ms";
            else
                HttpStatusLabel.Text = $"HTTP{i18N.Translate(": ", "Timeout")}";

            HttpStatusLabel.Visible = true;
        }
        finally
        {
            _httpConnectCts.Dispose();
            _httpConnectCts = null;
            HttpStatusLabel.Enabled = true;
        }
    }

    #endregion

    #endregion

    #region Misc

    #region PowerEvent

    private bool _resumeFlag;

    private void SystemEvents_PowerModeChanged(object sender, PowerModeChangedEventArgs e)
    {
        SystemEventsPowerModeChangedAsync(sender, e).ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Power mode changed handler failed");
        });
    }

    private async Task SystemEventsPowerModeChangedAsync(object sender, PowerModeChangedEventArgs e)
    {
        switch (e.Mode)
        {
            case PowerModes.Suspend: //操作系统即将挂起
                if (!IsWaiting())
                {
                    _resumeFlag = true;
                    Log.Information("OS Suspend, Stop");
                    await StopAsync();
                }

                break;
            case PowerModes.Resume: //操作系统即将从挂起状态继续
                if (_resumeFlag)
                {
                    _resumeFlag = false;
                    Log.Information("OS Resume, Restart");
                    ControlButton.PerformClick();
                }

                break;
        }
    }

    #endregion

    private void Minimize()
    {
        // 使关闭时窗口向右下角缩小的效果
        WindowState = FormWindowState.Minimized;

        if (_isFirstCloseWindow)
        {
            // 显示提示语
            NotifyTip(i18N.Translate("Hatch is now minimized to the notification bar, double click this icon to restore."));
            _isFirstCloseWindow = false;
        }

        Hide();
    }

    public void Exit(bool forceExit = false, bool saveConfiguration = true)
    {
        ExitAsync(forceExit, saveConfiguration).ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Exit failed");
        });
    }

    private async Task ExitAsync(bool forceExit = false, bool saveConfiguration = true)
    {
        if (!IsWaiting() && !Global.Settings.StopWhenExited && !forceExit)
        {
            MessageBoxX.Show(i18N.Translate("Please press Stop button first"));

            ShowMainFormToolStripButton.PerformClick();
            return;
        }

        // State = State.Terminating;
        NotifyIcon.Visible = false;
        Hide();

        if (saveConfiguration)
            await Configuration.SaveAsync();

        foreach (var file in new[] { Constants.TempConfig, Constants.TempRouteFile })
            if (File.Exists(file))
                File.Delete(file);

        await StopAsync();

        Dispose();
        Environment.Exit(Environment.ExitCode);
    }

    #region FormClosingButton

    private bool _isFirstCloseWindow = true;

    private void MainForm_FormClosing(object sender, FormClosingEventArgs e)
    {
        if (e.CloseReason == CloseReason.UserClosing && State != State.Terminating)
        {
            // 取消"关闭窗口"事件
            e.Cancel = true; // 取消关闭窗体

            // 如果未勾选关闭窗口时退出，隐藏至右下角托盘图标
            if (!Global.Settings.ExitWhenClosed)
                Minimize();
            // 如果勾选了关闭时退出，自动点击退出按钮
            else
                Exit();
        }
    }

    #endregion

    #region Updater

    private async Task CheckUpdateAsync()
    {
        try
        {
            await UpdateChecker.CheckAsync(Global.Settings.CheckBetaUpdate);
        }
        catch (Exception e)
        {
            Log.Warning(e, "Check update failed");
        }
    }

    private void UpdateChecker_NewVersionFound(object? sender, EventArgs e)
    {
        if (IsDisposed)
            return;

        BeginInvoke(() =>
        {
            NotifyTip(i18N.TranslateFormat("New version found: {0}", UpdateChecker.LatestVersionNumber));

            if (Flags.AlwaysShowNewVersionFound)
            {
                var result = MessageBoxX.Show(
                    i18N.TranslateFormat("New version found: {0}\nOpen release page now?", UpdateChecker.LatestVersionNumber),
                    LogLevel.INFO,
                    i18N.Translate("Update"),
                    true);

                if (result == DialogResult.OK)
                    Utils.Utils.Open(UpdateChecker.LatestVersionUrl);

                Flags.AlwaysShowNewVersionFound = false;
            }
        });
    }

    private void UpdateChecker_NewVersionNotFound(object? sender, EventArgs e)
    {
        if (!Flags.AlwaysShowNewVersionFound || IsDisposed)
            return;

        BeginInvoke(() =>
        {
            NotifyTip(i18N.Translate("Already the latest version"));
            Flags.AlwaysShowNewVersionFound = false;
        });
    }

    private void UpdateChecker_NewVersionFoundFailed(object? sender, EventArgs e)
    {
        if (!Flags.AlwaysShowNewVersionFound || IsDisposed)
            return;

        BeginInvoke(() =>
        {
            NotifyTip(i18N.Translate("Check update failed"), info: false);
            Flags.AlwaysShowNewVersionFound = false;
        });
    }

    #endregion

    #region NetTraffic

    /// <summary>
    ///     上一次下载的流量
    /// </summary>
    public ulong LastDownloadBandwidth;

    /// <summary>
    ///     上一次上传的流量
    /// </summary>
    public ulong LastUploadBandwidth;

    public void OnBandwidthUpdated(ulong download)
    {
        if (InvokeRequired)
        {
            BeginInvoke(() => OnBandwidthUpdated(download));
            return;
        }

        try
        {
            UsedBandwidthLabel.Text = $"{i18N.Translate("Used", ": ")}{Bandwidth.Compute(download)}";
            //UploadSpeedLabel.Text = $"↑: {Utils.Bandwidth.Compute(upload - LastUploadBandwidth)}/s";
            DownloadSpeedLabel.Text = $"↑↓: {Bandwidth.Compute(download - LastDownloadBandwidth)}/s";

            //LastUploadBandwidth = upload;
            LastDownloadBandwidth = download;
            Refresh();
        }
        catch (Exception e)
        {
            Log.Warning(e, "Failed to update bandwidth display");
        }
    }

    #endregion

    #region NotifyIcon

    private void ShowMainFormToolStripButton_Click(object sender, EventArgs e)
    {
        Utils.Utils.ActivateVisibleWindows();
    }

    /// <summary>
    ///     通知图标右键菜单退出
    /// </summary>
    private void ExitToolStripButton_Click(object sender, EventArgs e)
    {
        Exit();
    }

    private void NotifyIcon_MouseDoubleClick(object? sender, MouseEventArgs? e)
    {
        ShowMainFormToolStripButton.PerformClick();
    }

    public void NotifyTip(string text, int timeout = 0, bool info = true)
    {
        // 会阻塞线程 timeout 秒(?)
        NotifyIcon.ShowBalloonTip(timeout, UpdateChecker.Name, text, info ? ToolTipIcon.Info : ToolTipIcon.Error);
    }

    #endregion

    #region ComboBox_DrawItem

    private readonly SolidBrush _greenBrush = new(Color.FromArgb(50, 255, 56));
    private int _numberBoxWidth;
    private int _numberBoxX;
    private int _numberBoxWrap;

    private void ComboBox_DrawItem(object sender, DrawItemEventArgs e)
    {
        if (sender is not ComboBox cbx)
            return;

        // 绘制背景颜色
        e.Graphics.FillRectangle(Brushes.White, e.Bounds);

        if (e.Index < 0)
            return;

        // 绘制 备注/名称 字符串
        TextRenderer.DrawText(e.Graphics, cbx.Items[e.Index].ToString(), cbx.Font, e.Bounds, Color.Black, TextFormatFlags.Left);

        switch (cbx.Items[e.Index])
        {
            case Server item:
            {
                // 根据延迟显示不同颜色的文字，不使用背景色
                var delayColor = item.Delay switch
                {
                    > LatencyFairThresholdMs => Color.Red,
                    > LatencyGoodThresholdMs => Color.Orange,
                    >= 0 => Color.Green,
                    _ => Color.Gray
                };

                // 绘制延迟字符串（右对齐）
                var delayText = item.Delay >= 0 ? $"{item.Delay}ms" : (item.Delay == -2 ? "DNS" : "N/A");
                TextRenderer.DrawText(e.Graphics,
                    delayText,
                    cbx.Font,
                    new Point(_numberBoxX + _numberBoxWrap, e.Bounds.Y),
                    delayColor,
                    TextFormatFlags.Left);

                break;
            }
            case Mode item:
            {
                /*
                // 绘制 模式Box 底色
                e.Graphics.FillRectangle(Brushes.Gray, _numberBoxX, e.Bounds.Y, _numberBoxWidth, e.Bounds.Height);

                // 绘制 模式行数 字符串
                TextRenderer.DrawText(e.Graphics,
                    item.Content.Count.ToString(),
                    cbx.Font,
                    new Point(_numberBoxX + _numberBoxWrap, e.Bounds.Y),
                    Color.Black,
                    TextFormatFlags.Left);
                    */

                break;
            }
        }
    }

    #endregion

    #endregion
}
