using System;
using System.Collections;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Net.Sockets;
using System.Reflection;
using System.Text;
using System.Threading;

namespace Nodex.Trace
{
    /// <summary>
    /// Probes called by code woven with Nodex.Weaver. Every probe is a no-op unless the process
    /// was started by Nodex (environment variables <c>NODEX_TRACE</c> = "host:port" and
    /// <c>NODEX_TRACE_TOKEN</c>). Events are buffered and sent in batches by a background thread.
    /// </summary>
    /// <remarks>
    /// Wire format (all little-endian): handshake line <c>"NODEX1 {token}\n"</c>, then frames of
    /// <c>u32 length</c> + records. Record: <c>u8 kind, i32 thread, i64 micros, i32 method,
    /// i32 arg, u16 textLength, utf8 text</c>. Kinds: see <see cref="Kind"/>.
    /// Values are formatted here and never by calling property getters or user ToString().
    /// </remarks>
    public static class T
    {
        private enum Kind : byte
        {
            Enter = 1,
            Step = 2,
            Exit = 3,
            Local = 4,
            Return = 5,
            Arg = 6,
            Throw = 7,
            Dropped = 9,
        }

        private const int MaxPendingBytes = 16 * 1024 * 1024;
        private const int MaxText = 96;

        private static readonly bool s_on;
        private static readonly object s_lock = new object();
        private static readonly object s_flushLock = new object();
        private static MemoryStream s_buffer = new MemoryStream(64 * 1024);
        private static BinaryWriter s_writer = new BinaryWriter(s_buffer, new UTF8Encoding(false), leaveOpen: true);
        private static readonly Stopwatch s_clock = Stopwatch.StartNew();
        private static NetworkStream? s_stream;
        private static TcpClient? s_client;
        private static long s_dropped;

        static T()
        {
            var endpoint = Environment.GetEnvironmentVariable("NODEX_TRACE");
            var token = Environment.GetEnvironmentVariable("NODEX_TRACE_TOKEN");
            if (string.IsNullOrEmpty(endpoint) || string.IsNullOrEmpty(token)) return;
            try
            {
                var colon = endpoint!.LastIndexOf(':');
                var client = new TcpClient { NoDelay = true };
                client.Connect(endpoint.Substring(0, colon), int.Parse(endpoint.Substring(colon + 1), CultureInfo.InvariantCulture));
                s_client = client;
                s_stream = client.GetStream();
                var hello = Encoding.ASCII.GetBytes($"NODEX1 {token}\n");
                s_stream.Write(hello, 0, hello.Length);
                s_on = true;
                var flusher = new Thread(FlushLoop) { IsBackground = true, Name = "Nodex.Trace" };
                flusher.Start();
                AppDomain.CurrentDomain.ProcessExit += (_, __) => Close();
                AppDomain.CurrentDomain.FirstChanceException += (_, e) => Thrown(e.Exception);
                AppDomain.CurrentDomain.UnhandledException += (_, __) => Flush();
            }
            catch (Exception)
            {
                s_on = false; // Tracing must never break the program.
            }
        }

        public static void Enter(int method)
        {
            if (s_on) Record(Kind.Enter, method, 0, null);
        }

        public static void Step(int method, int point)
        {
            if (s_on) Record(Kind.Step, method, point, null);
        }

        public static void Exit(int method)
        {
            if (s_on) Record(Kind.Exit, method, 0, null);
        }

        /// <summary>Called with the value about to be returned; the weaver duplicates it first.</summary>
        public static void Return(object? value, int method)
        {
            if (s_on) Record(Kind.Return, method, 0, Format(value));
        }

        public static void Local(int method, int slot, object? value)
        {
            if (s_on) Record(Kind.Local, method, slot, Format(value));
        }

        public static void Arg(int method, int index, object? value)
        {
            if (s_on) Record(Kind.Arg, method, index, Format(value));
        }

        [ThreadStatic] private static bool t_inThrow;
        private static readonly FieldInfo? s_messageField =
            typeof(Exception).GetField("_message", BindingFlags.Instance | BindingFlags.NonPublic);

        /// <summary>
        /// Every exception at the moment it is thrown (before any catch runs). The server attaches it
        /// to the statement the thread is executing. The message is read from the base field, so a
        /// user <c>Message</c> override never runs.
        /// </summary>
        private static void Thrown(Exception exception)
        {
            if (!s_on || t_inThrow || Thread.CurrentThread.Name == "Nodex.Trace") return;
            t_inThrow = true;
            try
            {
                var message = s_messageField?.GetValue(exception) as string;
                var text = exception.GetType().Name + (string.IsNullOrEmpty(message) ? "" : ": " + message);
                Record(Kind.Throw, 0, 0, Truncate(text));
            }
            catch (Exception)
            {
                // never let tracing throw
            }
            finally
            {
                t_inThrow = false;
            }
        }

        private static void Record(Kind kind, int method, int arg, string? text)
        {
            var thread = Environment.CurrentManagedThreadId;
            var micros = s_clock.ElapsedTicks * 1_000_000 / Stopwatch.Frequency;
            lock (s_lock)
            {
                if (s_buffer.Length > MaxPendingBytes)
                {
                    s_dropped++;
                    return;
                }
                s_writer.Write((byte)kind);
                s_writer.Write(thread);
                s_writer.Write(micros);
                s_writer.Write(method);
                s_writer.Write(arg);
                if (text is null)
                {
                    s_writer.Write((ushort)0);
                }
                else
                {
                    var bytes = Encoding.UTF8.GetBytes(text);
                    s_writer.Write((ushort)bytes.Length);
                    s_writer.Write(bytes);
                }
            }
        }

        private static void FlushLoop()
        {
            while (true)
            {
                Thread.Sleep(15);
                Flush();
            }
        }

        /// Sends what is left and closes the connection gracefully, so the receiver does not
        /// lose buffered data to a connection reset when the process exits.
        private static void Close()
        {
            lock (s_flushLock)
            {
                FlushLocked();
                try
                {
                    s_client?.Client.Shutdown(SocketShutdown.Send);
                    s_client?.Close();
                }
                catch (Exception) { }
                s_stream = null;
            }
        }

        private static void Flush()
        {
            // One flusher at a time keeps frames in order (background thread vs. process exit).
            lock (s_flushLock) FlushLocked();
        }

        private static void FlushLocked()
        {
            byte[] payload;
            lock (s_lock)
            {
                if (s_dropped > 0)
                {
                    s_writer.Write((byte)Kind.Dropped);
                    s_writer.Write(0);
                    s_writer.Write(0L);
                    s_writer.Write(0);
                    s_writer.Write((int)Math.Min(s_dropped, int.MaxValue));
                    s_writer.Write((ushort)0);
                    s_dropped = 0;
                }
                if (s_buffer.Length == 0) return;
                payload = s_buffer.ToArray();
                s_buffer.SetLength(0);
            }
            try
            {
                var stream = s_stream;
                if (stream is null) return;
                var length = BitConverter.GetBytes((uint)payload.Length);
                stream.Write(length, 0, 4);
                stream.Write(payload, 0, payload.Length);
                stream.Flush();
            }
            catch (Exception)
            {
                s_stream = null; // Nodex went away; keep the program running.
            }
        }

        // ------------------------------------------------------------------------------------
        // Value formatting: bounded, and never runs user code (no getters, no user ToString()).
        // ------------------------------------------------------------------------------------

        private static readonly ConcurrentDictionary<Type, FieldInfo[]> s_fields = new ConcurrentDictionary<Type, FieldInfo[]>();

        internal static string Format(object? value) => Truncate(FormatValue(value, 1));

        private static string FormatValue(object? value, int depth)
        {
            switch (value)
            {
                case null:
                    return "null";
                case string s:
                    return "\"" + (s.Length > 48 ? s.Substring(0, 47) + "…" : s) + "\"";
                case bool b:
                    return b ? "true" : "false";
                case char c:
                    return "'" + c + "'";
                case Enum e:
                    return e.GetType().Name + "." + e;
                case IFormattable f when IsSystemType(value.GetType()):
                    return f.ToString(null, CultureInfo.InvariantCulture);
            }
            var type = value.GetType();
            var name = TypeName(type);
            if (IsSystemType(type))
            {
                return value is ICollection collection ? $"{name} (Count = {collection.Count})" : name;
            }
            if (depth <= 0) return name;
            // User types: read fields only (auto-properties are backing fields).
            var fields = s_fields.GetOrAdd(type, t => t.GetFields(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic));
            var sb = new StringBuilder(name).Append(" { ");
            var shown = 0;
            foreach (var field in fields)
            {
                if (shown == 4) { sb.Append("…  "); break; }
                var fieldName = field.Name;
                if (fieldName.StartsWith("<", StringComparison.Ordinal))
                {
                    var end = fieldName.IndexOf('>');
                    if (end < 0 || !fieldName.EndsWith("k__BackingField", StringComparison.Ordinal)) continue;
                    fieldName = fieldName.Substring(1, end - 1);
                }
                object? fieldValue;
                try { fieldValue = field.GetValue(value); } catch (Exception) { continue; }
                sb.Append(fieldName).Append(" = ").Append(FormatValue(fieldValue, depth - 1)).Append(", ");
                shown++;
            }
            if (shown == 0) return name;
            sb.Length -= 2;
            return sb.Append(" }").ToString();
        }

        private static bool IsSystemType(Type type) =>
            type.IsPrimitive || type == typeof(decimal) || (type.Namespace?.StartsWith("System", StringComparison.Ordinal) ?? false);

        private static string TypeName(Type type)
        {
            if (!type.IsGenericType) return type.Name;
            var tick = type.Name.IndexOf('`');
            var args = type.GetGenericArguments();
            var parts = new string[args.Length];
            for (var i = 0; i < args.Length; i++) parts[i] = TypeName(args[i]);
            return (tick > 0 ? type.Name.Substring(0, tick) : type.Name) + "<" + string.Join(", ", parts) + ">";
        }

        private static string Truncate(string text) => text.Length > MaxText ? text.Substring(0, MaxText - 1) + "…" : text;
    }
}
