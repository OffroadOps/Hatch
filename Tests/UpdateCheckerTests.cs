using Hatch.Controllers;
using Hatch.Models.GitHubRelease;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using System;

namespace Tests;

[TestClass]
public class UpdateCheckerTests
{
    [TestMethod]
    public void VersionComparer_AcceptsVPrefixedReleaseTags()
    {
        Assert.AreEqual(0, VersionUtil.CompareVersion("v2.0.0", "2.0.0"));
        Assert.IsTrue(VersionUtil.CompareVersion("v2.0.1", "2.0.0") > 0);
        Assert.IsTrue(VersionUtil.CompareVersion("v2.1.0-beta1", "v2.1.0") < 0);
        Assert.IsTrue(VersionUtil.CompareVersion("v2.0.0", "nightly") > 0);
    }

    [TestMethod]
    public void SelectLatestRelease_FiltersPrereleases()
    {
        var stable = new Release { tag_name = "v2.0.0", prerelease = false };
        var beta = new Release { tag_name = "v2.1.0-beta1", prerelease = true };

        Assert.AreSame(stable, UpdateChecker.SelectLatestRelease([stable, beta], false));
        Assert.AreSame(beta, UpdateChecker.SelectLatestRelease([stable, beta], true));
    }

    [TestMethod]
    public void SelectLatestRelease_RejectsEmptyReleaseList()
    {
        Assert.ThrowsException<InvalidOperationException>(() =>
            UpdateChecker.SelectLatestRelease([], false));
    }

    [TestMethod]
    public void ReleaseBody_ParsesChecksumsAndHidesChecksumSection()
    {
        const string hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        UpdateChecker.LatestRelease = new Release
        {
            body = $"# Hatch v2.0.0\n\nRelease notes.\n\n## 校验和 | Checksums\n\n| 文件名 | SHA256 |\n| :- | :- |\n| Hatch.zip | {hash} |"
        };

        var parsed = UpdateChecker.GetLatestUpdateFileNameAndHash("HATCH.ZIP");

        Assert.AreEqual("Hatch.zip", parsed.fileName);
        Assert.AreEqual(hash, parsed.sha256);
        StringAssert.Contains(UpdateChecker.GetLatestReleaseContent(), "Release notes.");
        Assert.IsFalse(UpdateChecker.GetLatestReleaseContent().Contains("Checksums", StringComparison.OrdinalIgnoreCase));
    }
}
