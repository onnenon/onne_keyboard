#![no_std]
#![no_main]

use panic_halt as _;

#[rtic::app(device = qt_py_m0::pac, peripherals = true)]
mod app {

    use bsp::hal::clock::GenericClockController;
    use bsp::hal::gpio::v2::{Input, Output, Pin, PullUp, PushPull, PA02, PA03};

    use bsp::hal;
    use bsp::pac;
    use qt_py_m0 as bsp;

    use cortex_m::peripheral::NVIC;
    use hal::usb::UsbBus;
    use pac::interrupt;
    use pac::CorePeripherals;
    use pac::Peripherals;
    use usb_device::bus::UsbBusAllocator;

    use keyberon::action::k;
    use keyberon::debounce::Debouncer;
    use keyberon::key_code::KbHidReport;
    use keyberon::key_code::KeyCode::Grave;
    use keyberon::layout::Layout;
    use keyberon::matrix::Matrix;
    use keyberon::matrix::PressedKeys;

    type UsbClass = keyberon::Class<'static, UsbBus, ()>;
    type UsbDev = usb_device::device::UsbDevice<'static, UsbBus>;

    pub static LAYERS: keyberon::layout::Layers = &[&[&[k(Grave)]]];

    #[shared]
    struct Shared {
        usb_dev: UsbDev,
        usb_class: UsbClass,
        debouncer: Debouncer<PressedKeys<1, 1>>,
        layout: Layout,
        matrix: Matrix<Pin<PA02, Input<PullUp>>, Pin<PA03, Output<PushPull>>, 1, 1>,
        // timer: TimerCounter<TC3>,
    }

    #[local]
    struct Local {}

    #[init]
    fn init(_ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        static mut USB_ALLOCATOR: Option<UsbBusAllocator<UsbBus>> = None;
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

        let usb_class = keyberon::new_class(bus_allocator, ());
        let usb_dev = keyberon::new_device(bus_allocator);

        let debouncer = Debouncer::new(PressedKeys::default(), PressedKeys::default(), 5);

        let matrix = Matrix::new(
            [pins.analog.a0.into_pull_up_input(); 1],
            [pins.analog.a1.into_push_pull_output(); 1],
        )
        .unwrap();

        let layout = Layout::new(LAYERS);

        unsafe {
            core.NVIC.set_priority(interrupt::USB, 1);
            NVIC::unmask(interrupt::USB);
        }

        (
            Shared {
                debouncer,
                layout,
                matrix,
                usb_class,
                usb_dev,
            },
            Local {},
            init::Monotonics(),
        )
    }

    #[task(binds = TC3, shared= [usb_class, debouncer, matrix, layout])]
    fn tc3(ctx: tc3::Context) {
        let debouncer = ctx.shared.debouncer;
        let usb_class = ctx.shared.usb_class;
        let matrix = ctx.shared.matrix;
        let layout = ctx.shared.layout;

        (debouncer, usb_class, matrix, layout).lock(|debouncer, usb_class, matrix, layout| {
            for event in debouncer.events(matrix.get().unwrap()) {
                layout.event(event);
            }
            let report: KbHidReport = layout.keycodes().collect();
            while let Ok(0) = usb_class.write(report.as_bytes()) {}
        });
    }

    #[task(binds = USB, priority = 2, shared = [usb_class, usb_dev])]
    fn usb(ctx: usb::Context) {
        let usb_class = ctx.shared.usb_class;
        let usb_dev = ctx.shared.usb_dev;

        (usb_class, usb_dev).lock(|usb_class, usb_dev| usb_dev.poll(&mut [usb_class]));
    }
}
