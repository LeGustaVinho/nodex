export function scriptCollections() {
  const items = [1, 2];
  const fifo = [1];
  const enqueue = fifo;
  enqueue.push(2);
  fifo.shift();
  console.log(fifo.length);
  {
    const fifo = [1];
    fifo.pop();
  }
  const lifo = [1];
  lifo.push(2);
  lifo.pop();
  const mixed = [1];
  mixed.push(2);
  mixed.shift();
  mixed.pop();
  const escaped = [1];
  const leaked = escaped;
  escaped.push(2);
  escaped.shift();
  console.log(leaked);
  const loopUpdate = [1];
  loopUpdate.push(2);
  loopUpdate.shift();
  for (let i = 0; i < 1; i++, loopUpdate.pop()) {}
  const captured = [1];
  captured.push(2);
  captured.shift();
  function removeLast() { captured.pop(); }
  removeLast();
  const changedLength = [1];
  changedLength.push(2);
  changedLength.shift();
  changedLength.length = 0;
  return items;
}

export function scriptChangingTypes(flag) {
  let x = 1;
  beforeWrite(x);
  x = "text";
  afterWrite(x);
  if (flag) { x = "another"; }
  else { x = false; }
  afterBranch(x);
  x = unknownValue();
  afterUnknown(x);
}
