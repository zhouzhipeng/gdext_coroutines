#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes)]
#![feature(unboxed_closures)]

use std::time::Duration;
use async_compat::Compat;
use gdext_coroutines::prelude::*;
use godot::classes::node::ProcessMode;
use godot::obj::WithBaseField;
use godot::prelude::*;

struct IntegrationTests;

#[gdextension]
unsafe impl ExtensionLibrary for IntegrationTests {}

#[derive(GodotClass)]
#[class(init, base = Node)]
struct TestClass {
	base: Base<Node>,
}

#[godot_api]
impl TestClass {
	#[func]
	fn test_routine(&mut self) -> Gd<SpireCoroutine> {
		self.start_async_task(Compat::new(
			async {
				godot_print!("Using compat layer!");
				smol::Timer::after(Duration::from_secs(2)).await;
				godot_print!("Awaited 2 seconds, returning 5");
				5_i32
			}))
	}
	
	#[func]
	fn test_from_other_node(node: Gd<Node>) -> Gd<SpireCoroutine> {
		node.start_async_task(Compat::new(
			async {
				godot_print!("Async task from other node!");
				smol::Timer::after(Duration::from_secs(2)).await;
				godot_print!("Awaited 2 seconds, returning `finished task`");
				"finished task"
			}))
	}
}

#[godot_api]
impl INode for TestClass {
	fn ready(&mut self) {
		let base = self.base().to_godot();
		test_1(base);
	}
}

fn log(msg: impl std::fmt::Display) {
	godot_print!("[{:.6}] {msg}", godot::classes::Engine::singleton().get_process_frames());
}

fn log_err(msg: impl std::fmt::Display) {
	godot_print_rich!("[color=red]ERROR[/color]: [{:.6}] {msg}", godot::classes::Engine::singleton().get_process_frames());
}

fn test_1(node: Gd<Node>) {
	log("Starting test 1");

	let first_routine =
		node.start_coroutine(
			#[coroutine] || {
				log("1st Coroutine started");

				let engine = godot::classes::Engine::singleton();

				{
					let start_frame = engine.get_process_frames() as i64;

					yield frames(2);

					let current_frame = engine.get_process_frames() as i64;

					let frame_diff = current_frame - start_frame;
					if frame_diff != 2 {
						log_err(format!("Expected 2 frames to have passed, got: {frame_diff}"));
					}
				}

				{
					let start_frame = engine.get_process_frames() as i64;

					yield frames(0);

					let current_frame = engine.get_process_frames() as i64;

					let frame_diff = current_frame - start_frame;
					if frame_diff != 0 {
						log_err(format!("Expected 0 frames to have passed, got: {frame_diff}"));
					}
				}

				{
					let time = godot::classes::Time::singleton();

					let start_time = time.get_ticks_msec() as i64;

					yield seconds(1.5);

					let current_time = time.get_ticks_msec() as i64;

					let time_passed = current_time - start_time;
					log(format!("Time passed after 1.5 seconds yield: {time_passed} ms"));
				}

				{
					let start_frame = engine.get_process_frames() as i64;
					let frame_end = start_frame as u64 + 60;

					let moved_engine = engine.clone();
					yield wait_until(
						move || moved_engine.get_process_frames() >= frame_end);

					let current_frame = engine.get_process_frames() as i64;

					let frame_diff = current_frame - start_frame;
					if frame_diff != 60 {
						log_err(format!("Expected 60 frames to have passed, got: {frame_diff}"));
					}
				}

				{
					let start_frame = engine.get_process_frames() as i64;
					let frame_end = start_frame as u64 + 100;

					let moved_engine = engine.clone();
					yield wait_while(
						move || moved_engine.get_process_frames() < frame_end);

					let current_frame = engine.get_process_frames() as i64;

					let frame_diff = current_frame - start_frame;
					if frame_diff != 100 {
						log_err(format!("Expected 100 frames to have passed, got: {frame_diff}"));
					}
				}

				log("1st Coroutine finished");
			});

	let node_ref = node.clone();

	node.coroutine(
		#[coroutine] move || {
			log("2nd Coroutine started. Waiting for 1st before continuing...");

			if !first_routine.is_running() {
				log_err("1st Coroutine not running");
			}

			yield first_routine.wait_until_finished();

			if !first_routine.is_finished() {
				log_err("1st Coroutine not finished");
			}

			log("Test 1 finished");

			test_2(node_ref);
		})
	    .auto_start(true)
	    .process_mode(ProcessMode::INHERIT)
	    .spawn();
}

fn test_2(node: Gd<Node>) {
	log("Starting test 2");

	let mut paused_routine =
		node.coroutine(
			#[coroutine] || {
				log("Paused routine started");

				yield frames(10);

				log("Paused routine finished");
			})
		    .auto_start(false)
		    .spawn();

	let node_ref = node.clone();

	node.start_coroutine(
		#[coroutine] move || {
			log("Auto started routine!");

			log("Resuming paused routine, then waiting for it to finish.");

			let mut bind = paused_routine.bind_mut();
			bind.resume();
			drop(bind);

			yield paused_routine.wait_until_finished();

			log("Test 2 finished");

			test_3(node_ref);
		});
}

fn test_3(node: Gd<Node>) {
	log("Starting test 3");

	let mut frames_routine =
		node.start_coroutine(
			#[coroutine] || {
				log("Frames routine started");

				let mut frame_count = 0;

				loop {
					yield frames(1);
					frame_count += 1;
					log(format!("Frames routine frame count: {frame_count}"));

					if frame_count >= 6000 {
						log("Frames routine finished");
						break;
					}
				}
			});

	let node_ref = node.clone();

	node.start_coroutine(
		#[coroutine] move || {
			log("Auto started routine");

			log("Pausing frames routine");

			{
				let mut bind = frames_routine.bind_mut();
				bind.pause();
			}

			yield seconds(1.0);

			log("Resuming frames routine");

			{
				let mut bind = frames_routine.bind_mut();
				bind.resume();
			}

			yield seconds(0.5);

			log("Stopping frames routine");

			{
				let mut bind = frames_routine.bind_mut();
				bind.kill();
			}

			yield frames(1);

			if frames_routine.is_running() {
				log_err("Frames routine still running after stopping");
			}

			if !frames_routine.is_finished() {
				log_err("Frames routine not finished after stopping");
			}

			log("Test 3 finished");

			test_4(node_ref);
		});
}

fn test_4(node: Gd<Node>) {
	log("Starting test 4");

	log("Pausing Scene Tree");

	node.get_tree().unwrap().set_pause(true);

	let mut inherit_routine =
		node.coroutine(
			#[coroutine] move || {
				log_err("Inherit routine still running after stopping processing");

				yield frames(5);

				log_err("Inherit routine finished");
			})
		    .auto_start(true)
		    .process_mode(ProcessMode::INHERIT)
		    .spawn();

	let node_ref = node.clone();

	node.coroutine(
		#[coroutine] move || {
			log("Always coroutine started");

			yield frames(50);

			log("Always coroutine finished");

			{
				let mut bind = inherit_routine.bind_mut();
				bind.kill();
			}

			log("Resuming Scene Tree");

			node_ref.get_tree().unwrap().set_pause(false);

			log("Test 4 finished");

			test_5(node_ref);
		})
	    .auto_start(true)
	    .process_mode(ProcessMode::ALWAYS)
	    .spawn();

	node.coroutine(
		#[coroutine] move || {
			log_err("False auto_start routine is running despite not being started");

			yield seconds(1.0);

			log_err("False auto_start routine finished");
		})
	    .auto_start(false)
	    .process_mode(ProcessMode::INHERIT)
	    .spawn();
	
	node.start_coroutine(
		#[coroutine] move || {
			panic!("Testing panic behavior in coroutine, this message should appear in the godot error log.");
			#[allow(unreachable_code)]
			()
		});
}

fn test_5(node: Gd<Node>) {
	log("Starting test 5");

	let mut coroutine =
		node.start_coroutine(
			#[coroutine] || {
				log("Starting really long coroutine");

				yield seconds(1000.0);

				log("Really long coroutine finished");
			});

	coroutine.bind_mut().force_run_to_completion();

	let mut coroutine_with_return =
		node.start_coroutine(
			#[coroutine] || {
				yield frames(1);

				"Hello world"
			});

	let ret = coroutine_with_return.bind_mut().force_run_to_completion();
	log(format!("Returned value: `{ret}`"));

	node.coroutine(
		#[coroutine] || {
			yield frames(5);
			5_i32
		})
	    .on_finish(|x| {
		    log(format!("Returned value: {x}"))
	    })
	    .spawn();

	node.start_async_task(
		async {
			log("Async coroutine started");

			smol::Timer::after(Duration::from_secs(10)).await;

			log("Async coroutine finished");
		});

	node.coroutine(
		#[coroutine] || {
			yield frames(2);
			5.0
		})
	    .on_finish_callable(Callable::from_fn("anon",
		    |args| {
			    match args.first() {
				    Some(var) => log(format!("Args: {var:?}")),
				    None => log_err("Args array is empty"),
			    }

			    log("Test 5 finished");

			    Ok(Variant::nil())
		    }))
	    .spawn()
	    .bind_mut()
	    .finish_with(5_i32.to_variant());
}