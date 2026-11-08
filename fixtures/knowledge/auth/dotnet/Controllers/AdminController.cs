using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Auth.Services;

namespace Auth.Controllers;

[ApiController]
[Route("api/[controller]")]
[Authorize(Roles = "Admin,Ops")]
public class AdminController : ControllerBase
{
    private readonly IReportService _reports;

    public AdminController(IReportService reports)
    {
        _reports = reports;
    }

    [HttpGet("reports")]
    public IActionResult Reports()
    {
        _reports.RecordAccess("reports");
        return Ok(_reports.Build());
    }

    [HttpPost("purge")]
    [Authorize(Policy = "CanPurge")]
    public IActionResult Purge(bool audit)
    {
        if (audit)
        {
            _reports.RecordAccess("purge");
        }
        return NoContent();
    }

    [HttpGet("health")]
    [AllowAnonymous]
    public IActionResult Health() => Ok();
}
