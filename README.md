# Chartacritty = Alacritty plus charts

![Chartacritty running nvim inside tmux](https://user-images.githubusercontent.com/873436/77846106-de3f5a80-71b3-11ea-87fe-ad054e76d319.png)

## About
This is a modified version of Alacritty that includes drawing time series charts.

I use this to monitor kubernetes clusters, for example alerts and number of workers.
This is not a mature project. It's my learning steps for rust and async.

## How it works

A background thread is started for tokio, it is in charge of:
- asynchronously loading metrics from remote sources (prometheus for now)
- using timers to load data in intervals
- maintaining a representation of the loaded data in OpenGL and serving it to clients

## Last merge date from upstream/master (real alacritty)
- 2020-03-26 (fde2424)

## License

Alacritty is released under the [Apache License, Version 2.0].

[Apache License, Version 2.0]: https://github.com/alacritty/alacritty/blob/master/LICENSE-APACHE
[tmux]: https://github.com/tmux/tmux
