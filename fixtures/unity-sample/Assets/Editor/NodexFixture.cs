using System;
using UnityEditor;

/// For Nodex's end-to-end tests, which run this project in a batch-mode Editor: the Editor stays
/// up (the Nodex bridge answers the test) and quits by itself after NODEX_FIXTURE_SECONDS, so a
/// failed test never leaves it running.
[InitializeOnLoad]
internal static class NodexFixture
{
    static NodexFixture()
    {
        var seconds = Environment.GetEnvironmentVariable("NODEX_FIXTURE_SECONDS");
        if (!UnityEngine.Application.isBatchMode || string.IsNullOrEmpty(seconds) || !double.TryParse(seconds, out var limit)) return;
        if (SessionState.GetFloat("nodex.fixture.deadline", 0) == 0)
        {
            SessionState.SetFloat("nodex.fixture.deadline", (float)(EditorApplication.timeSinceStartup + limit));
        }
        EditorApplication.update += () =>
        {
            if (EditorApplication.timeSinceStartup > SessionState.GetFloat("nodex.fixture.deadline", 0)) EditorApplication.Exit(0);
        };
    }

    /// `-executeMethod NodexFixture.Stay`: returns at once; the Editor keeps running (no -quit).
    public static void Stay()
    {
    }
}
