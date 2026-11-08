using System;
using Microsoft.AspNetCore.Mvc;

namespace Review.Controllers;

[ApiController]
[Route("api/widgets")]
public class WidgetsController : ControllerBase
{
    [HttpGet]
    [Route("{id}")]
    public IActionResult Get(int id) => Ok(id);

    [HttpPost("{id}/archive")]
    [HttpPut]
    [Route("{id}/state")]
    public IActionResult Change(int id) => Ok();

    [HttpDelete("{id}")]
    public IActionResult Remove(int id)
    {
        try
        {
            throw new InvalidOperationException("locked");
        }
        catch (InvalidOperationException)
        {
            return Conflict();
        }
    }

    [HttpPatch("{id}")]
    public IActionResult Rename(int id)
    {
        throw new ArgumentException("name");
    }
}
