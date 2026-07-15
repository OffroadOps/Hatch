using Hatch.Utils;

namespace Hatch.Forms.ModeForms;

public sealed class WindowsStoreAppSelectForm : Form
{
    private readonly IReadOnlyList<WindowsStoreAppInfo> _apps;
    private readonly TextBox _searchTextBox = new();
    private readonly CheckedListBox _appListBox = new();

    public IReadOnlyList<WindowsStoreAppInfo> SelectedApps { get; private set; } = Array.Empty<WindowsStoreAppInfo>();

    public WindowsStoreAppSelectForm(IReadOnlyList<WindowsStoreAppInfo> apps)
    {
        _apps = apps;

        InitializeComponent();
        ApplyFilter();
    }

    private void InitializeComponent()
    {
        Text = "选择 Windows 应用";
        StartPosition = FormStartPosition.CenterParent;
        MinimizeBox = false;
        MaximizeBox = false;
        ShowIcon = false;
        Width = 760;
        Height = 520;
        MinimumSize = new Size(600, 420);

        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 3,
            Padding = new Padding(12)
        };
        layout.RowStyles.Add(new RowStyle());
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        layout.RowStyles.Add(new RowStyle());

        _searchTextBox.Dock = DockStyle.Top;
        _searchTextBox.PlaceholderText = "搜索应用、包名、可执行文件...";
        _searchTextBox.TextChanged += (_, _) => ApplyFilter();

        _appListBox.CheckOnClick = true;
        _appListBox.Dock = DockStyle.Fill;
        _appListBox.HorizontalScrollbar = true;
        _appListBox.DisplayMember = nameof(WindowsStoreAppInfo.Title);

        var buttonPanel = new FlowLayoutPanel
        {
            Dock = DockStyle.Fill,
            FlowDirection = FlowDirection.RightToLeft,
            AutoSize = true
        };

        var okButton = new Button { Text = "确定", DialogResult = DialogResult.OK, Width = 88 };
        var cancelButton = new Button { Text = "取消", DialogResult = DialogResult.Cancel, Width = 88 };
        var selectAllButton = new Button { Text = "全选", Width = 88 };
        var clearButton = new Button { Text = "清空", Width = 88 };

        okButton.Click += (_, _) =>
        {
            SelectedApps = _appListBox.CheckedItems.Cast<WindowsStoreAppInfo>().ToList();
        };
        selectAllButton.Click += (_, _) => SetAllVisibleChecked(true);
        clearButton.Click += (_, _) => SetAllVisibleChecked(false);

        buttonPanel.Controls.Add(okButton);
        buttonPanel.Controls.Add(cancelButton);
        buttonPanel.Controls.Add(selectAllButton);
        buttonPanel.Controls.Add(clearButton);

        layout.Controls.Add(_searchTextBox, 0, 0);
        layout.Controls.Add(_appListBox, 0, 1);
        layout.Controls.Add(buttonPanel, 0, 2);

        AcceptButton = okButton;
        CancelButton = cancelButton;
        Controls.Add(layout);
    }

    private void ApplyFilter()
    {
        var selectedRules = _appListBox.CheckedItems
            .Cast<WindowsStoreAppInfo>()
            .Select(app => app.ToProcessRule())
            .ToHashSet(StringComparer.OrdinalIgnoreCase);

        var keyword = _searchTextBox.Text.Trim();
        var filtered = _apps.Where(app =>
            keyword.IsNullOrWhiteSpace()
            || app.Title.Contains(keyword, StringComparison.CurrentCultureIgnoreCase)
            || app.PackageFullName.Contains(keyword, StringComparison.OrdinalIgnoreCase)
            || app.PackageFamilyName.Contains(keyword, StringComparison.OrdinalIgnoreCase)
            || app.InstallLocation.Contains(keyword, StringComparison.OrdinalIgnoreCase));

        _appListBox.BeginUpdate();
        _appListBox.Items.Clear();
        foreach (var app in filtered)
        {
            var index = _appListBox.Items.Add(app);
            _appListBox.SetItemChecked(index, selectedRules.Contains(app.ToProcessRule()));
        }
        _appListBox.EndUpdate();
    }

    private void SetAllVisibleChecked(bool value)
    {
        for (var i = 0; i < _appListBox.Items.Count; i++)
            _appListBox.SetItemChecked(i, value);
    }
}
