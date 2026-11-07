import { Component } from "react";

type State = { now: Date };

/** A class component, as written before hooks. */
export class Clock extends Component<{ label: string }, State> {
  state: State = { now: new Date() };
  private timer?: ReturnType<typeof setInterval>;

  componentDidMount() {
    this.timer = setInterval(() => this.tick(), 1000);
  }

  componentWillUnmount() {
    clearInterval(this.timer);
  }

  tick() {
    this.setState({ now: new Date() });
  }

  render() {
    return (
      <time>
        {this.props.label} {this.state.now.toLocaleTimeString()}
      </time>
    );
  }
}
