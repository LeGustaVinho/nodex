using System;

namespace Sample.Domain
{
    public enum OrderStatus
    {
        Pending,
        Paid,
        Shipped,
        Cancelled,
    }

    public record Order(int Id, string Customer, decimal Amount, OrderStatus Status)
    {
        public bool IsLarge => Amount >= 100m;

        public Order WithStatus(OrderStatus status) => this with { Status = status };
    }

    public interface IEntity
    {
        int Id { get; }
    }

    public abstract class Shape : IEntity
    {
        public int Id { get; init; }

        public abstract double Area();

        public virtual string Describe()
        {
            return $"{GetType().Name} with area {Area():0.00}";
        }
    }

    public sealed class Circle : Shape
    {
        private readonly double _radius;

        public Circle(double radius)
        {
            if (radius <= 0)
            {
                throw new ArgumentOutOfRangeException(nameof(radius));
            }
            _radius = radius;
        }

        public override double Area() => Math.PI * _radius * _radius;
    }

    public sealed class Rectangle : Shape
    {
        public double Width { get; set; }
        public double Height { get; set; }

        public override double Area()
        {
            return Width * Height;
        }
    }
}
