#![no_std]
#![no_main]

use panic_halt as _;

use bsp::hal;
use bsp::pac;
use qt_py_m0 as bsp;

use bsp::hal::usb::UsbBus;
use core::convert::Infallible;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use hal::clock::GenericClockController;
use hal::gpio::v2::{PA02, PA03};
use hal::gpio::{Input, Output, PullUp, PushPull};
use hal::prelude::*;
use hal::timer;
use hal::timer::TimerCounter;
use pac::CorePeripherals;
use pac::Peripherals;
use rtic::app;

use keyberon::action::Action::{self};
use keyberon::action::{k, m};
use keyberon::debounce::Debouncer;
use keyberon::key_code::KeyCode::*;
use keyberon::key_code::{KbHidReport, KeyCode};
use keyberon::layout::Layout;
use keyberon::matrix::{Matrix, PressedKeys};

type UsbClass = keyberon::Class<'static, UsbBus, ()>;
type UsbDevice = usb_device::device::UsbDevice<'static, UsbBus>;

pub struct Cols(pub hal::gpio::v2::pin::Pin<PA02, Input<PullUp>>);

pub struct Rows(pub hal::gpio::v2::pin::Pin<PA03, Output<PushPull>>);

const CUT: Action = m(&[LShift, Delete]);

pub static LAYERS: keyberon::layout::Layers = &[&[&[k(Grave)]]];

#[app(device = qt_py_m0::pac)]
const APP: () = {
    struct Resources {
        usb_dev: UsbDevice,
        usb_class: UsbClass,
        matrix: Matrix<dyn InputPin<Error = Infallible>, dyn OutputPin<Error = Infallible>, 1, 1>,
        debouncer: Debouncer<PressedKeys<1, 1>>,
        layout: Layout,
        timer: timer::TimerCounter3,
    }

    #[init]
    fn init(mut c: init::Context) -> init::LateResources {
        let mut peripherals = Peripherals::take().unwrap();
        let mut core = CorePeripherals::take().unwrap();

        let mut clocks = GenericClockController::with_internal_32kosc(
            peripherals.GCLK,
            &mut peripherals.PM,
            &mut peripherals.SYSCTRL,
            &mut peripherals.NVMCTRL,
        );

        let pins = bsp::Pins::new(peripherals.PORT).split();

        let usb_bus = Some(
            pins.usb
                .init(peripherals.USB, &mut clocks, &mut peripherals.PM),
        )
        .as_ref()
        .unwrap();

        let usb_class = keyberon::new_class(usb_bus, ());
        let usb_dev = keyberon::new_device(usb_bus);

        let usb_class = keyberon::new_class(usb_bus, ());
        let usb_dev = keyberon::new_device(usb_bus);

        let gclk0 = clocks.gclk0();
        let timer_clock = clocks.tcc2_tc3(&gclk0).unwrap();
        let mut timer = TimerCounter::tc3_(&timer_clock, peripherals.TC3, &mut peripherals.PM);
        timer.start(3.mhz());

        let matrix = Matrix::new(
            Cols(pins.analog.a0.into_pull_up_input()),
            Rows(pins.analog.a1.into_push_pull_output()),
        );

        init::LateResources {
            usb_dev,
            usb_class,
            timer,
            debouncer: Debouncer::new(PressedKeys::default(), PressedKeys::default(), 5),
            matrix: matrix.unwrap(),
            layout: Layout::new(LAYERS),
        }
    }

    #[task(binds = USB, priority = 2, resources = [usb_dev, usb_class])]
    fn usb_rx(mut c: usb_rx::Context) {
        usb_poll(&mut c.resources.usb_dev, &mut c.resources.usb_class);
    }

    #[task(binds = TC3, priority = 1, resources = [usb_class, matrix, debouncer, layout, timer])]
    fn tick(mut c: tick::Context) {
        for event in c
            .resources
            .debouncer
            .events(c.resources.matrix.get().unwrap())
        {
            send_report(c.resources.layout.event(event), &mut c.resources.usb_class);
        }
        send_report(c.resources.layout.tick(), &mut c.resources.usb_class);
    }
};

fn send_report(iter: impl Iterator<Item = KeyCode>, usb_class: &mut resources::usb_class<'_>) {
    use rtic::Mutex;
    let report: KbHidReport = iter.collect();
    if usb_class.lock(|k| k.device_mut().set_keyboard_report(report.clone())) {
        while let Ok(0) = usb_class.lock(|k| k.write(report.as_bytes())) {}
    }
}

fn usb_poll(usb_dev: &mut UsbDevice, keyboard: &mut UsbClass) {
    if usb_dev.poll(&mut [keyboard]) {
        keyboard.poll();
    }
}
