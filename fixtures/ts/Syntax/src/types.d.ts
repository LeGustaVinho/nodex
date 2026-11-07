declare const VERSION: string;

declare namespace Api {
  interface Response {
    status: number;
  }
  function call(url: string): Promise<Response>;
}

declare function globalHelper(x: number): void;
