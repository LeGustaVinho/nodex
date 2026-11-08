#nullable enable

using System;
using System.IO;
using System.Linq;
using Nodex.Trace;
using UnityEditor;
using UnityEditor.Compilation;
using UnityEngine;

namespace Nodex.Bridge
{
    /// <summary>
    /// What Nodex asks of this Editor, through files in the project (no network listener here):
    /// <list type="bullet">
    /// <item><c>.nodex/unity/session.json</c> (written by Nodex): the trace intake to send Play
    /// Mode events to, whether to press Play, whether to switch to Debug code optimization.</item>
    /// <item><c>Library/Nodex/trace-on</c> (written by Nodex): the scripts must be compiled with
    /// trace probes; the bridge recompiles when this differs from what is loaded.</item>
    /// <item><c>.nodex/unity/editor.json</c> (written here): the Editor's state, for Nodex.</item>
    /// </list>
    /// </summary>
    [InitializeOnLoad]
    internal static class NodexBridge
    {
        public const string Version = "0.1.0";

        [Serializable]
        private sealed class Session
        {
            public string id = "";
            public string endpoint = "";
            public string token = "";
            public bool play;
            public bool debug;
        }

        [Serializable]
        private sealed class EditorState
        {
            public string package = Version;
            public string unity = "";
            public int pid;
            public bool playing;
            public bool compiling;
            public string codeOptimization = "";
            public bool traceCompiled;
            public bool traceRequested;
            public bool connected;
            public string session = "";
            public long updated;
        }

        private static readonly string Root = Directory.GetCurrentDirectory();
        private static readonly string SessionFile = Path.Combine(Root, ".nodex", "unity", "session.json");
        private static readonly string StateFile = Path.Combine(Root, ".nodex", "unity", "editor.json");
        private static readonly string TraceFlag = Path.Combine(Root, "Library", "Nodex", "trace-on");

        /// Whether the loaded script assemblies carry probes (they reference Nodex.Trace).
        private static readonly bool TraceCompiled;
        private static double s_nextTick;
        private static double s_nextBeat;
        private static string s_lastState = "";

        static NodexBridge()
        {
            TraceCompiled = AppDomain.CurrentDomain.GetAssemblies().Any(a =>
            {
                var name = a.GetName().Name;
                if (name.StartsWith("Nodex.", StringComparison.Ordinal) || name.StartsWith("Unity", StringComparison.Ordinal) || name.StartsWith("System", StringComparison.Ordinal)) return false;
                try
                {
                    return a.GetReferencedAssemblies().Any(r => r.Name == "Nodex.Trace");
                }
                catch (Exception)
                {
                    return false;
                }
            });
            EditorApplication.update += Tick;
            EditorApplication.playModeStateChanged += OnPlayModeChanged;
            AssemblyReloadEvents.beforeAssemblyReload += T.Disconnect;
            EditorApplication.quitting += T.Disconnect;
            // Entering Play Mode reloads the domain: connect before the first Awake.
            if (EditorApplication.isPlayingOrWillChangePlaymode) TryConnect(ReadSession());
        }

        private static Session? ReadSession()
        {
            try
            {
                if (!File.Exists(SessionFile)) return null;
                var s = JsonUtility.FromJson<Session>(File.ReadAllText(SessionFile));
                return string.IsNullOrEmpty(s?.id) ? null : s;
            }
            catch (Exception)
            {
                return null; // being written: next tick
            }
        }

        private static void TryConnect(Session? s)
        {
            if (s == null || string.IsNullOrEmpty(s.endpoint) || T.IsConnected) return;
            T.Connect(s.endpoint, s.token);
        }

        private static void OnPlayModeChanged(PlayModeStateChange change)
        {
            switch (change)
            {
                case PlayModeStateChange.ExitingEditMode:
                case PlayModeStateChange.EnteredPlayMode:
                    TryConnect(ReadSession());
                    break;
                case PlayModeStateChange.ExitingPlayMode:
                    T.Disconnect();
                    break;
            }
        }

        private static void Tick()
        {
            var now = EditorApplication.timeSinceStartup;
            if (now < s_nextTick) return;
            s_nextTick = now + 0.5;
            var session = ReadSession();
            var traceOn = File.Exists(TraceFlag);
            var busy = EditorApplication.isCompiling || EditorApplication.isUpdating;

            // Probes in or out: recompile once per change of the flag.
            if (traceOn != TraceCompiled && !busy && !EditorApplication.isPlayingOrWillChangePlaymode)
            {
                var want = traceOn ? "on" : "off";
                if (SessionState.GetString("nodex.recompiledFor", "") != want)
                {
                    SessionState.SetString("nodex.recompiledFor", want);
                    Debug.Log($"Nodex: recompiling scripts with trace probes {want}");
                    CompilationPipeline.RequestScriptCompilation(RequestScriptCompilationOptions.CleanBuildCache);
                }
            }

            if (session != null && session.debug && CompilationPipeline.codeOptimization != CodeOptimization.Debug)
            {
                Debug.Log("Nodex: switching the Editor to Debug mode for debugging");
                CompilationPipeline.codeOptimization = CodeOptimization.Debug;
            }

            // Play requests: once per Nodex session, after the probes are compiled in.
            var played = SessionState.GetString("nodex.played", "");
            var ready = !busy && (!traceOn || TraceCompiled);
            if (session != null && session.play && played != session.id && ready && !EditorApplication.isPlayingOrWillChangePlaymode)
            {
                SessionState.SetString("nodex.played", session.id);
                EditorApplication.EnterPlaymode();
            }
            else if ((session == null || !session.play) && played != "" && EditorApplication.isPlaying)
            {
                SessionState.SetString("nodex.played", "");
                EditorApplication.ExitPlaymode();
            }

            if (EditorApplication.isPlaying) TryConnect(session);
            if (T.IsConnected && (session == null || string.IsNullOrEmpty(session.endpoint))) T.Disconnect();

            WriteState(session, traceOn, now);
        }

        private static void WriteState(Session? session, bool traceOn, double now)
        {
            var state = new EditorState
            {
                unity = Application.unityVersion,
                pid = System.Diagnostics.Process.GetCurrentProcess().Id,
                playing = EditorApplication.isPlaying,
                compiling = EditorApplication.isCompiling,
                codeOptimization = CompilationPipeline.codeOptimization.ToString(),
                traceCompiled = TraceCompiled,
                traceRequested = traceOn,
                connected = T.IsConnected,
                session = session?.id ?? "",
            };
            var text = JsonUtility.ToJson(state);
            // Rewritten on change, and every 2 s as a heartbeat.
            if (text == s_lastState && now < s_nextBeat) return;
            s_lastState = text;
            s_nextBeat = now + 2;
            state.updated = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(StateFile)!);
                var temp = StateFile + ".tmp";
                File.WriteAllText(temp, JsonUtility.ToJson(state));
                if (File.Exists(StateFile)) File.Delete(StateFile);
                File.Move(temp, StateFile);
            }
            catch (Exception)
            {
                // Nodex reads it again later.
            }
        }
    }
}
