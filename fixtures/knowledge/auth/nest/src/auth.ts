export class JwtAuthGuard {}
export class RolesGuard {}
export const Roles = (...roles: string[]) => (target: object) => target;
export const Public = () => (target: object) => target;
