#![no_std]
#![no_main]

use bsp::hal::gpio::v2::Pin;
use panic_halt as _;

use core::convert::Infallible;
use embedded_hal::digital::v2::{InputPin, OutputPin};

use bsp::hal;
use bsp::pac;
use qt_py_m0 as bsp;

use cortex_m::asm::wfi;
use cortex_m::peripheral::NVIC;
use usb_device::bus::UsbBusAllocator;
use usb_device::prelude::*;

use bsp::entry;
use hal::clock::GenericClockController;
use hal::usb::UsbBus;
use pac::interrupt;
use pac::CorePeripherals;
use pac::Peripherals;

use hal::gpio::v2::Input;
use hal::gpio::v2::Output;
use hal::gpio::v2::PullUp;
use hal::gpio::v2::PushPull;
use hal::gpio::v2::PA02;
use hal::gpio::v2::PA03;

use keyberon::action::k;
use keyberon::debounce::Debouncer;
// use keyberon::key_code::KbHidReport;
// use keyberon::key_code::KeyCode;
use keyberon::key_code::KeyCode::Grave;
use keyberon::layout::Layout;
use keyberon::matrix::Matrix;
use keyberon::matrix::PressedKeys;

// type UsbClass = HidClass<'static, UsbBus, Keyboard<()>>;
// type UsbClass = keyberon::Class<'static, UsbBus, ()>;
// type UsbDev = UsbDevice<'static, UsbBus>;

pub struct Cols(pub hal::gpio::v2::pin::Pin<PA02, Input<PullUp>>);
// impl_heterogenous_array! {
//     Cols,
//     dyn InputPin<Error = Infallible>,
//     U1,
//     [0]
// }

pub struct Rows(pub hal::gpio::v2::pin::Pin<PA03, Output<PushPull>>);
// impl_heterogenous_array! {
//     Rows,
//     dyn OutputPin<Error = Infallible>,
//     U1,
//     [0]
// }
pub static LAYERS: keyberon::layout::Layers = &[&[&[k(Grave)]]];

#[entry]
fn main() -> ! {
    let mut peripherals = Peripherals::take().unwrap();
    let mut core = CorePeripherals::take().unwrap();
    let mut clocks = GenericClockController::with_internal_32kosc(
        peripherals.GCLK,
        &mut peripherals.PM,
        &mut peripherals.SYSCTRL,
        &mut peripherals.NVMCTRL,
    );
    let pins = bsp::Pins::new(peripherals.PORT).split();

    let bus_allocator = unsafe {
        USB_ALLOCATOR = Some(
            pins.usb
                .init(peripherals.USB, &mut clocks, &mut peripherals.PM),
        );
        USB_ALLOCATOR.as_ref().unwrap()
    };

    unsafe {
        USB_CLASS = Some(keyberon::new_class(bus_allocator, ()));
        USB_DEV = Some(keyberon::new_device(bus_allocator));
        LAYOUT = Some(Layout::new(LAYERS));
        DEBOUNCER = Some(Debouncer::new(
            PressedKeys::default(),
            PressedKeys::default(),
            5,
        ));
        MATRIX = Matrix::new(
            [pins.analog.a0.into_pull_up_input(); 1],
            [pins.analog.a1.into_push_pull_output(); 1],
        )
        .ok();
    }

    unsafe {
        core.NVIC.set_priority(interrupt::USB, 1);
        NVIC::unmask(interrupt::USB);
    }

    loop {
        wfi();
    }
}

static mut USB_ALLOCATOR: Option<UsbBusAllocator<UsbBus>> = None;
static mut USB_CLASS: Option<keyberon::Class<'static, UsbBus, ()>> = None;
static mut USB_DEV: Option<UsbDevice<UsbBus>> = None;
static mut LAYOUT: Option<keyberon::layout::Layout> = None;
static mut DEBOUNCER: Option<Debouncer<PressedKeys<1, 1>>> = None;
static mut MATRIX: Option<Matrix<Pin<PA02, Input<PullUp>>, Pin<PA03, Output<PushPull>>, 1, 1>> =
    None;

fn poll_usb() {
    unsafe {
        USB_DEV.as_mut().map(|usb_dev| {
            USB_CLASS.as_mut().map(|keyboard| {
                usb_dev.poll(&mut [keyboard]);

                // for event in DEBOUNCER
                //     .as_mut()
                //     .unwrap()
                //     .events(MATRIX.as_mut().unwrap().get().unwrap())
                // {
                //     let report: KbHidReport = LAYOUT.as_mut().unwrap().event(event).collect();
                //     while let Ok(0) = keyboard.write(report.as_bytes()) {}
                // }
                // let report: KbHidReport = LAYOUT.as_mut().unwrap().tick().collect();
                // while let Ok(0) = keyboard.write(report.as_bytes()) {}
            });
        });
    };
    // unsafe { DEBOUNCER.unwrap().events(MATRIX.unwrap().get().unwrap()) };
}

#[interrupt]
fn USB() {
    poll_usb();
}
