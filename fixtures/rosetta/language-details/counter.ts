export class Counter {
  readonly settings = { step: 1 };
  private current = 0;
  #limit = 10;
}
