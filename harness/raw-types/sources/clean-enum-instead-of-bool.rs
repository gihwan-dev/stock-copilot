pub enum OrderSwitch {
    On,
    Off,
}

pub fn switch(left: &Price, right: &Price) -> OrderSwitch {
    if left == right {
        OrderSwitch::On
    } else {
        OrderSwitch::Off
    }
}
