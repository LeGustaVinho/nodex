namespace States;

/// <summary>The State pattern: each state is a class that chooses the next one.</summary>
public interface ILightState
{
    void Next(TrafficLight light);
}

public class Red : ILightState
{
    public void Next(TrafficLight light)
    {
        light.SetState(new Green());
    }
}

public class Green : ILightState
{
    public void Next(TrafficLight light)
    {
        light.SetState(new Yellow());
    }
}

public class Yellow : ILightState
{
    public void Next(TrafficLight light)
    {
        light.SetState(new Red());
    }
}

public class TrafficLight
{
    private ILightState _state = new Red();

    public void SetState(ILightState state)
    {
        _state = state;
    }

    public void Tick()
    {
        _state.Next(this);
    }
}
