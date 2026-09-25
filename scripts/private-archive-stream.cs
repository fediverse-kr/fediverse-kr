// Resource guard for the archive library's destination stream. No ZIP parsing
// or cryptography is implemented here: SharpZipLib checks the ZIP data and
// .NET supplies the SHA-256 used by the private extraction inventory.
using System;
using System.Diagnostics;
using System.IO;
using System.Security.Cryptography;

namespace Fedkr.PrivateBackup
{
    public sealed class BoundedWriteStream : Stream
    {
        private readonly Stream destination;
        private readonly long maximum;
        private readonly Stopwatch clock;
        private readonly TimeSpan budget;
        private readonly IncrementalHash hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        private bool disposed;
        private long written;

        public BoundedWriteStream(Stream destination, long maximum, Stopwatch clock, TimeSpan budget)
        {
            if (destination == null || !destination.CanWrite || maximum < 0 ||
                clock == null || !clock.IsRunning || budget <= TimeSpan.Zero)
                throw new ArgumentException("Invalid archive output guard.");
            this.destination = destination;
            this.maximum = maximum;
            this.clock = clock;
            this.budget = budget;
        }

        public long BytesWritten => written;
        public string HexSha256()
        {
            if (disposed) throw new ObjectDisposedException(nameof(BoundedWriteStream));
            return Convert.ToHexString(hash.GetCurrentHash()).ToLowerInvariant();
        }
        public override bool CanRead => false;
        public override bool CanSeek => false;
        public override bool CanWrite => !disposed;
        public override long Length => written;
        public override long Position
        {
            get => written;
            set => throw new NotSupportedException();
        }

        private void Check(int count)
        {
            if (disposed) throw new ObjectDisposedException(nameof(BoundedWriteStream));
            if (count < 0 || count > maximum - written || clock.Elapsed > budget)
                throw new InvalidDataException("Archive output limit exceeded.");
        }
        public override void Write(byte[] buffer, int offset, int count)
        {
            if (buffer == null) throw new ArgumentNullException(nameof(buffer));
            if (offset < 0 || count < 0 || offset > buffer.Length - count)
                throw new ArgumentOutOfRangeException();
            Write(new ReadOnlySpan<byte>(buffer, offset, count));
        }
        public override void Write(ReadOnlySpan<byte> buffer)
        {
            Check(buffer.Length);
            destination.Write(buffer);
            hash.AppendData(buffer);
            written += buffer.Length;
        }
        public override void Flush()
        {
            Check(0);
            destination.Flush();
        }
        public override int Read(byte[] buffer, int offset, int count) => throw new NotSupportedException();
        public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
        public override void SetLength(long value) => throw new NotSupportedException();
        protected override void Dispose(bool disposing)
        {
            if (disposing && !disposed) { hash.Dispose(); disposed = true; }
            // The caller owns the FileStream and performs Flush(true)/Dispose.
            base.Dispose(disposing);
        }
    }
}
