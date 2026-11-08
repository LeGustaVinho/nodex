using Stateless;

namespace States;

public enum DoorState { Open, Closed, Locked, Broken }
public enum DoorTrigger { Close, Open, Lock, Unlock, Kick }

public class Door
{
    private readonly StateMachine<DoorState, DoorTrigger> _machine = new StateMachine<DoorState, DoorTrigger>(DoorState.Closed);

    public Door()
    {
        _machine.Configure(DoorState.Open)
            .Permit(DoorTrigger.Close, DoorState.Closed);

        _machine.Configure(DoorState.Closed)
            .Permit(DoorTrigger.Open, DoorState.Open)
            .PermitIf(DoorTrigger.Lock, DoorState.Locked, () => HasKey);

        _machine.Configure(DoorState.Locked)
            .SubstateOf(DoorState.Closed)
            .Permit(DoorTrigger.Unlock, DoorState.Closed)
            .Permit(DoorTrigger.Kick, DoorState.Broken);
    }

    public bool HasKey { get; set; }

    public void Fire(DoorTrigger trigger) => _machine.Fire(trigger);
}
