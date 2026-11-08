using Microsoft.AspNetCore.Mvc;

namespace Review.Controllers;

[ApiController]
[Route("api/[controller]/[action]")]
public class ItemsController : ControllerBase
{
    [HttpGet]
    public IActionResult List() => Ok();
}
