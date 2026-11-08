import { Controller, Delete, Get, Injectable, Param, UseGuards } from "@nestjs/common";
import { JwtAuthGuard, Public, Roles, RolesGuard } from "./auth";

@Injectable()
export class UsersService {
  constructor(private readonly audit: AuditLog) {}

  list(): string[] {
    this.audit.write("list");
    return [];
  }
}

@Injectable()
export class AuditLog {
  write(what: string): void {}
}

@Controller("users")
@UseGuards(JwtAuthGuard, RolesGuard)
export class UsersController {
  constructor(private readonly users: UsersService) {}

  @Get()
  list() {
    return this.users.list();
  }

  @Delete(":id")
  @Roles("admin")
  remove(@Param("id") id: string) {
    return id;
  }

  @Get("public/info")
  @Public()
  info() {
    return "info";
  }
}
