function typed(depth) {
    let x = "start";
    const items = [x];
    if (depth) typed(depth - 1);
    x = 42;
    return x;
}

console.log(typed(2));
