using System.Collections.Generic;
using UnityEngine;

namespace Sample
{
    public enum OrderStatus
    {
        Pending,
        Paid,
        Cancelled,
    }

    public sealed class Order
    {
        public int Id;
        public string Customer;
        public int Amount;
        public OrderStatus Status;

        public Order(int id, string customer, int amount, OrderStatus status)
        {
            Id = id;
            Customer = customer;
            Amount = amount;
            Status = status;
        }
    }

    /// Sums alice's orders every frame, like the other Samples' TotalForCustomer.
    public class OrderLoop : MonoBehaviour
    {
        // Created when Play Mode starts, so no scene has to reference it.
        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
        private static void Spawn()
        {
            new GameObject("Orders").AddComponent<OrderLoop>();
        }

        private readonly List<Order> orders = new List<Order>
        {
            new Order(1, "alice", 120, OrderStatus.Paid),
            new Order(2, "bob", 80, OrderStatus.Pending),
            new Order(3, "alice", 75, OrderStatus.Paid),
            new Order(4, "alice", 40, OrderStatus.Cancelled),
        };

        public int frames;
        public int lastTotal;

        private void Update()
        {
            frames++;
            lastTotal = TotalForCustomer("alice");
        }

        public int TotalForCustomer(string customer)
        {
            var total = 0;
            foreach (var order in orders)
            {
                if (order.Customer != customer)
                {
                    continue;
                }
                if (order.Status == OrderStatus.Cancelled)
                {
                    continue;
                }
                total += order.Amount;
            }
            return total;
        }
    }
}
