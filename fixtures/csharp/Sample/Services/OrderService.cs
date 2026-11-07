using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Sample.Domain;

namespace Sample.Services
{
    public interface IOrderRepository
    {
        IEnumerable<Order> All();
        Order? Find(int id);
        void Add(Order order);
        void Replace(Order order);
    }

    public class InMemoryOrderRepository : IOrderRepository
    {
        private readonly Dictionary<int, Order> _orders = new();
        private readonly object _gate = new();

        public IEnumerable<Order> All() => _orders.Values;

        public Order? Find(int id)
        {
            return _orders.TryGetValue(id, out var order) ? order : null;
        }

        public void Add(Order order)
        {
            lock (_gate)
            {
                _orders[order.Id] = order;
            }
        }

        public void Replace(Order order) => _orders[order.Id] = order;
    }

    public class OrderService
    {
        private readonly IOrderRepository _repository;

        public event Action<Order>? OrderShipped;

        public OrderService(IOrderRepository repository)
        {
            _repository = repository ?? throw new ArgumentNullException(nameof(repository));
        }

        public decimal TotalForCustomer(string customer)
        {
            decimal total = 0;
            foreach (var order in _repository.All())
            {
                if (order.Customer != customer)
                {
                    continue;
                }

                if (order.Status == OrderStatus.Cancelled)
                {
                    break;
                }

                total += order.Amount;
            }
            return total;
        }

        public decimal TotalForCustomerLinq(string customer)
        {
            return _repository.All()
                .Where(o => o.Customer == customer)
                .Select(o => o.Amount)
                .Sum();
        }

        public IEnumerable<string> DescribeAll()
        {
            for (int i = 0; i < 3; i++)
            {
                var order = _repository.Find(i + 1);
                string label = order?.Status switch
                {
                    OrderStatus.Paid => "paid",
                    OrderStatus.Pending => "waiting",
                    null => "missing",
                    _ => "other",
                };
                yield return $"#{i + 1}: {label}";
            }
        }

        public async Task ShipAsync(int id)
        {
            var order = _repository.Find(id) ?? throw new InvalidOperationException($"Order {id} not found");
            if (order.Status != OrderStatus.Paid)
            {
                throw new InvalidOperationException("Only paid orders can ship");
            }

            await Task.Delay(10);
            var shipped = order.WithStatus(OrderStatus.Shipped);
            _repository.Replace(shipped);
            OrderShipped?.Invoke(shipped);
        }

        public int CountPending()
        {
            int count = 0;
            using (var scope = new AuditScope("count"))
            {
                var all = _repository.All().ToList();
                int index = 0;
                while (index < all.Count)
                {
                    if (all[index].Status is OrderStatus.Pending)
                    {
                        count++;
                    }
                    index++;
                }
            }
            return count;
        }
    }

    public sealed class AuditScope : IDisposable
    {
        private readonly string _name;

        public AuditScope(string name)
        {
            _name = name;
        }

        public void Dispose()
        {
            Console.WriteLine($"audit: {_name}");
        }
    }
}
