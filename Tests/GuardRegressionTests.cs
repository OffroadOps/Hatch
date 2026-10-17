using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using Hatch.Controllers;
using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Tests;

[TestClass]
public class GuardRegressionTests
{
    private sealed class OutputGuard : Guard
    {
        private readonly string _path;
        public OutputGuard(string executable, string path)
            : base(Path.GetRelativePath(Path.GetFullPath("bin"), executable))
        {
            _path = path;
            Instance.StartInfo.WorkingDirectory = Path.GetTempPath();
        }
        public override string Name => "output-regression";
        protected override string LogPath => _path;
        public Task RunAsync() => StartGuardAsync("--guard-output");
    }

    [TestMethod]
    public async Task ConcurrentOutput_IsDrainedBeforeRepeatedStop()
    {
        var executable = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../Native/NativeSocksTests.exe"));
        if (!File.Exists(executable)) Assert.Inconclusive("Build NativeSocksTests.vcxproj to run the child-process regression.");
        var logPath = Path.Combine(Path.GetTempPath(), "hatch-guard-" + Guid.NewGuid().ToString("N") + ".log");
        var guard = new OutputGuard(executable, logPath);
        try
        {
            await guard.RunAsync();
            await guard.Instance.WaitForExitAsync().WaitAsync(TimeSpan.FromSeconds(10));
            await Task.WhenAll(guard.StopAsync(), guard.StopAsync()).WaitAsync(TimeSpan.FromSeconds(10));
            var lines = await File.ReadAllLinesAsync(logPath);
            Assert.AreEqual(2000, lines.Length);
            Assert.AreEqual(1000, lines.Count(s => s.StartsWith("stdout-", StringComparison.Ordinal)));
            Assert.AreEqual(1000, lines.Count(s => s.StartsWith("stderr-", StringComparison.Ordinal)));
            Assert.AreEqual(2000, lines.Distinct().Count());
        }
        finally
        {
            await guard.StopAsync();
            File.Delete(logPath);
        }
    }
}
