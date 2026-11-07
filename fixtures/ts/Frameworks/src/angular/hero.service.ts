import { Injectable, inject, signal, computed } from "@angular/core";
import { HttpClient } from "@angular/common/http";
import { Observable, map, filter, catchError, of } from "rxjs";

export interface Hero {
  id: number;
  name: string;
}

@Injectable({ providedIn: "root" })
export class HeroService {
  private readonly http = inject(HttpClient);
  readonly selected = signal<Hero | null>(null);
  readonly title = computed(() => this.selected()?.name ?? "No hero");

  heroes(): Observable<Hero[]> {
    return this.http.get<Hero[]>("/api/heroes").pipe(
      map((heroes) => heroes.filter((h) => h.name.length > 0)),
      filter((heroes) => heroes.length > 0),
      catchError(() => of([])),
    );
  }

  select(hero: Hero): void {
    this.selected.set(hero);
  }
}
