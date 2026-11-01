// Copyright 2026 YUME contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::platform::windows::{EventLoopBuilderExtWindows, WindowExtWindows};
use std::cell::RefCell;

struct NestedPaint {
  target: HWND,
  sent: Cell<bool>,
}

unsafe extern "system" fn nested_paint(
  hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _: usize, data: usize,
) -> LRESULT {
  let state = &*(data as *const NestedPaint);
  if msg == WM_PAINT && !state.sent.replace(true) {
    SendMessageW(state.target, WM_PAINT, Some(WPARAM(0)), Some(LPARAM(0)));
  }
  DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[test]
fn nested_thread_paint_keeps_outer_redraw_owner_and_processes_next_cycle() {
  let event_loop = crate::event_loop::EventLoopBuilder::<()>::new()
    .with_any_thread(true)
    .build();
  let window = crate::window::WindowBuilder::new()
    .with_visible(false)
    .build(&event_loop)
    .unwrap();
  let hwnd = HWND(window.hwnd() as _);
  let runner = &event_loop.event_loop.window_target.p.runner_shared;
  let seen = Rc::new(RefCell::new(Vec::new()));
  let seen_handler = seen.clone();
  let state = NestedPaint { target: runner.thread_msg_target(), sent: Cell::new(false) };
  unsafe {
    runner.set_event_handler(move |event, flow| {
      *flow = ControlFlow::Wait;
      match event {
        Event::MainEventsCleared => seen_handler.borrow_mut().push("main"),
        Event::RedrawRequested(_) => seen_handler.borrow_mut().push("redraw"),
        Event::RedrawEventsCleared => seen_handler.borrow_mut().push("cleared"),
        Event::UserEvent(()) => seen_handler.borrow_mut().push("user"),
        _ => (),
      }
    });
    assert!(SetWindowSubclass(hwnd, Some(nested_paint), 9876, &state as *const _ as usize).as_bool());
    runner.poll();
    // A window procedure can synchronously dispatch a paint for the thread
    // target while the outer flush is delivering that window's WM_PAINT.
    runner.main_events_cleared();
    SendMessageW(hwnd, WM_PAINT, Some(WPARAM(0)), Some(LPARAM(0)));
    let panic = runner.take_panic_error();
    if panic.is_ok() { runner.redraw_events_cleared(); }
    let first = seen.borrow().clone();
    if panic.is_ok() {
      runner.send_event(Event::UserEvent(()));
      runner.main_events_cleared();
      SendMessageW(hwnd, WM_PAINT, Some(WPARAM(0)), Some(LPARAM(0)));
      runner.redraw_events_cleared();
    }
    let second = runner.take_panic_error();
    assert!(RemoveWindowSubclass(hwnd, Some(nested_paint), 9876).as_bool());
    runner.reset_runner();
    assert!(state.sent.get(), "native nested WM_PAINT was not exercised");
    assert!(panic.is_ok(), "nested WM_PAINT panicked in the native Tao callback");
    assert!(second.is_ok(), "subsequent event cycle panicked");
    assert_eq!(first, ["main", "redraw", "cleared"]);
    assert_eq!(*seen.borrow(), ["main", "redraw", "cleared", "user", "main", "redraw", "cleared"]);
  }
}
