using System;
using System.Threading.Tasks;
using Sample.Domain;
using Sample.Services;

namespace Sample;

public static class Program
{
    public static async Task Main(string[] args)
    {
        var repository = new InMemoryOrderRepository();
        var service = new OrderService(repository);
        service.OrderShipped += order => Console.WriteLine($"Shipped {order.Id}");

        repository.Add(new Order(1, "alice", 120m, OrderStatus.Paid));
        repository.Add(new Order(2, "bob", 40m, OrderStatus.Pending));
        repository.Add(new Order(3, "alice", 75m, OrderStatus.Paid));

        decimal total = service.TotalForCustomer("alice");
        Console.WriteLine($"Total: {total}");

        foreach (var label in service.DescribeAll())
        {
            Console.WriteLine(label);
        }

        try
        {
            await service.ShipAsync(2);
        }
        catch (InvalidOperationException ex)
        {
            Console.WriteLine($"Could not ship: {ex.Message}");
        }
    }
}
