using System;
using System.IO;
using System.Text.Json;
using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Tests;

/// <summary>
/// The "update bundled cores" flow in AboutForm downloads xray/sing-box and
/// installs them next to the executable, where they are later launched as child
/// processes. The archive must therefore be checked against the SHA256 digest
/// published by the GitHub releases API before it is extracted. These tests pin
/// the parsing of that digest: anything that is not a full sha256 hex value has
/// to yield an empty string so the download is refused instead of trusted.
/// </summary>
[TestClass]
public class CoreDownloadIntegrityTests
{
    private const string Valid = "d004c39288ce9ada487c6f398c7c545f7d749e44bdfdd59dbc9f865afba4e1ad";

    // Resolves Hatch.Utils.Utils explicitly: the bare name is shadowed by the
    // Hatch.Utils namespace once that namespace is in scope.
    private static string Parse(string? digest) => Hatch.Utils.Utils.ParseSha256Digest(digest);

    [TestMethod]
    public void PositiveDigest_IsParsedAndNormalized()
    {
        Assert.AreEqual(Valid, Parse("sha256:" + Valid));
        // GitHub returns lowercase, but accept and normalize any casing.
        Assert.AreEqual(Valid, Parse("sha256:" + Valid.ToUpperInvariant()));
        Assert.AreEqual(Valid, Parse("SHA256:" + Valid));
    }

    [TestMethod]
    public void MissingOrUnsupportedDigest_IsRejected()
    {
        // No digest published by the API: refuse rather than install unverified.
        Assert.AreEqual("", Parse(null));
        Assert.AreEqual("", Parse(""));
        Assert.AreEqual("", Parse("   "));
        // Non-sha256 algorithms must not be silently treated as sha256.
        Assert.AreEqual("", Parse("sha512:" + Valid));
        Assert.AreEqual("", Parse(Valid));
        Assert.AreEqual("", Parse("sha256:"));
        Assert.AreEqual("", Parse("sha256:not-hex"));
    }

    [TestMethod]
    public void TruncatedOrOverlongDigest_IsRejected()
    {
        Assert.AreEqual("", Parse("sha256:" + Valid[..63]));
        Assert.AreEqual("", Parse("sha256:" + Valid + "0"));
        Assert.AreEqual("", Parse("sha256:" + Valid[..32]));
    }

    /// <summary>
    /// The pinned cores in build.dependencies.json are hashed with the same
    /// algorithm, so a digest taken from the releases API must round-trip to the
    /// value the build already trusts for the same release asset.
    /// </summary>
    [TestMethod]
    public void PinnedXrayReleaseDigest_MatchesPublishedApiDigest()
    {
        var manifest = FindManifest();
        Assert.IsNotNull(manifest, "build.dependencies.json not found above the test output");

        using var document = JsonDocument.Parse(File.ReadAllText(manifest));
        var pinned = document.RootElement.GetProperty("xray").GetProperty("sha256").GetString()!;

        Assert.AreEqual(64, pinned.Length);
        Assert.AreEqual(pinned, Parse("sha256:" + pinned));
    }

    /// <summary>
    /// Walks up from the test output directory so the lookup does not depend on
    /// how many path segments the target framework adds.
    /// </summary>
    private static string? FindManifest()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory != null)
        {
            var candidate = Path.Combine(directory.FullName, "build.dependencies.json");
            if (File.Exists(candidate))
                return candidate;

            directory = directory.Parent;
        }

        return null;
    }
}
