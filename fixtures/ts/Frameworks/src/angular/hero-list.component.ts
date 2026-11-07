import { Component, EventEmitter, HostListener, Input, OnDestroy, OnInit, Output } from "@angular/core";
import { Subscription } from "rxjs";
import { Hero, HeroService } from "./hero.service.js";

@Component({
  selector: "app-hero-list",
  template: `<li *ngFor="let hero of heroes" (click)="choose(hero)">{{ hero.name }}</li>`,
})
export class HeroListComponent implements OnInit, OnDestroy {
  @Input() limit = 10;
  @Output() chosen = new EventEmitter<Hero>();
  heroes: Hero[] = [];
  private subscription?: Subscription;

  constructor(private readonly service: HeroService) {}

  ngOnInit(): void {
    this.subscription = this.service.heroes().subscribe((heroes) => {
      this.heroes = heroes.slice(0, this.limit);
    });
  }

  ngOnDestroy(): void {
    this.subscription?.unsubscribe();
  }

  @HostListener("window:keydown.escape")
  clear(): void {
    this.heroes = [];
  }

  choose(hero: Hero): void {
    this.service.select(hero);
    this.chosen.emit(hero);
  }
}
