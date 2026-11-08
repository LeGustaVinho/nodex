namespace States;

public enum OrderStatus { Draft, Submitted, Paid, Shipped, Cancelled, Archived }

/// <summary>An order and its life cycle (enum state with guard clauses).</summary>
public class Order
{
    public OrderStatus Status { get; private set; } = OrderStatus.Draft;
    public decimal Total { get; private set; }

    public void Submit()
    {
        if (Status != OrderStatus.Draft)
            throw new InvalidOperationException("Only drafts can be submitted");
        Status = OrderStatus.Submitted;
    }

    public void Pay(decimal amount)
    {
        EnsureStatus(OrderStatus.Submitted);
        if (amount < Total)
            return;
        Status = OrderStatus.Paid;
    }

    public void Ship()
    {
        switch (Status)
        {
            case OrderStatus.Paid:
                Status = OrderStatus.Shipped;
                break;
            case OrderStatus.Draft:
            case OrderStatus.Submitted:
                throw new InvalidOperationException("Not paid yet");
        }
    }

    public void Cancel()
    {
        if (Status == OrderStatus.Shipped || Status == OrderStatus.Cancelled)
            return;
        Status = OrderStatus.Cancelled;
    }

    private void EnsureStatus(OrderStatus expected)
    {
        if (Status != expected)
            throw new InvalidOperationException($"Expected {expected}");
    }
}
