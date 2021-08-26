#![no_std]
#![no_main]

use panic_halt as _;

use bsp::hal;
use bsp::pac;
use qt_py_m0 as bsp;

use bsp::entry;
use cortex_m::asm::wfi;
use cortex_m::peripheral::NVIC;
use hal::clock::GenericClockController;
use hal::usb::UsbBus;
use pac::interrupt;
use pac::CorePeripherals;
use pac::Peripherals;
use usb_device::bus::UsbBusAllocator;
use usb_device::prelude::*;

use keyberon::action::k;
use keyberon::debounce::Debouncer;
use keyberon::key_code::KbHidReport;
use keyberon::key_code::KeyCode::Grave;
use keyberon::layout::Layout;
use keyberon::matrix::Matrix;
use keyberon::matrix::PressedKeys;

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
    }
    let mut debouncer: Debouncer<PressedKeys<1, 1>> =
        Debouncer::new(PressedKeys::default(), PressedKeys::default(), 5);

    let mut matrix = Matrix::new(
        [pins.analog.a0.into_pull_up_input(); 1],
        [pins.analog.a1.into_push_pull_output(); 1],
    )
    .unwrap();

    let mut layout = Layout::new(LAYERS);

    unsafe {
        core.NVIC.set_priority(interrupt::USB, 1);
        NVIC::unmask(interrupt::USB);
    }

    loop {
        for event in debouncer.events(matrix.get().unwrap()) {
            layout.event(event);
        }
        let report: KbHidReport = layout.keycodes().collect();
        unsafe { while let Ok(0) = USB_CLASS.as_mut().unwrap().write(report.as_bytes()) {} }
        wfi();
    }
}

static mut USB_ALLOCATOR: Option<UsbBusAllocator<UsbBus>> = None;
static mut USB_CLASS: Option<keyberon::Class<'static, UsbBus, ()>> = None;
static mut USB_DEV: Option<UsbDevice<UsbBus>> = None;

fn poll_usb() {
    unsafe {
        USB_DEV.as_mut().map(|usb_dev| {
            USB_CLASS.as_mut().map(|keyboard| {
                usb_dev.poll(&mut [keyboard]);
            });
        });
    };
}

#[interrupt]
fn USB() {
    poll_usb();
}
