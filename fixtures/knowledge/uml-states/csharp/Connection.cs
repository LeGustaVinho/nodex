namespace States;

/// <summary>Boolean flags that together make a state.</summary>
public class Connection : IDisposable
{
    private bool _connected;
    private bool _disposed;

    public void Open()
    {
        if (_disposed)
            throw new ObjectDisposedException(nameof(Connection));
        _connected = true;
    }

    public void Close()
    {
        _connected = false;
    }

    public void Dispose()
    {
        _connected = false;
        _disposed = true;
    }
}
