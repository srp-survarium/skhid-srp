//! The options' Microphone list (docs/notes/voice-chat.md, "Microphone"):
//! the capture devices it shows, and which WinMM device push-to-talk and the
//! microphone test open for the player's choice. What Windows says is read
//! in `windows.rs`; this is the part the tests run with a made-up list.
use voice_abi::{Microphone, MicrophoneId};

/// A WinMM capture device as Windows lists it now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    /// Its endpoint ID (WinMM's `DRV_QUERYFUNCTIONINSTANCEID`): None when
    /// WinMM gives none, or one that is not an ID.
    pub id: Option<MicrophoneId>,
    /// WinMM's name of it (`waveInGetDevCaps`: cut at 31 characters).
    pub short: String,
    /// Its endpoint's whole name (`PKEY_Device_FriendlyName`), when Windows
    /// gives it.
    pub full: Option<String>,
}

/// The list the options show: each device with an endpoint ID, in Windows'
/// order, by its whole name, else WinMM's, else its ID; at most `room`'s
/// length. How many were written.
pub fn list(found: &[Found], room: &mut [Microphone]) -> usize {
    let listed = found.iter().filter_map(|device| {
        let id = device.id?;
        let id_text = std::str::from_utf8(id.as_bytes()).unwrap_or("");
        let name = [device.full.as_deref(), Some(device.short.as_str()), Some(id_text)]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|name| !name.is_empty())
            .unwrap_or(id_text);
        Some(Microphone::new(id, name))
    });
    let mut count = 0;
    for (slot, microphone) in room.iter_mut().zip(listed) {
        *slot = microphone;
        count += 1;
    }
    count
}

/// The capture device to open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    /// Windows' default (WinMM's mapper): no choice, or the chosen device is
    /// not there.
    Default,
    /// WinMM's device of this number.
    Number(u32),
}

/// The device of the `chosen` endpoint among WinMM's devices now (`ids`,
/// by their numbers), else Windows' default.
pub fn device(chosen: &MicrophoneId, ids: &[Option<MicrophoneId>]) -> Device {
    ids.iter()
        .position(|id| id.as_ref() == Some(chosen))
        .and_then(|number| u32::try_from(number).ok())
        .map_or(Device::Default, Device::Number)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> MicrophoneId {
        MicrophoneId::parse(text.as_bytes()).unwrap()
    }

    fn found(id_text: Option<&str>, short: &str, full: Option<&str>) -> Found {
        Found { id: id_text.map(id), short: short.into(), full: full.map(Into::into) }
    }

    fn names(room: &[Microphone], count: usize) -> Vec<(String, String)> {
        room[..count]
            .iter()
            .map(|microphone| {
                let (id, name) = microphone.read().expect("a listed device reads back");
                (String::from_utf8(id.as_bytes().to_vec()).unwrap(), name.to_str().unwrap().to_owned())
            })
            .collect()
    }

    #[test]
    fn the_list_is_windows_order_by_whole_names() {
        let windows = [
            // WinMM's 31 characters, and the endpoint's whole name
            found(Some("{0.0.1.00000000}.{a}"), "Microphone (Realtek(R) Audio) ", Some("Microphone (Realtek(R) Audio) Front Panel")),
            // no whole name (MMDevice not there): WinMM's
            found(Some("{0.0.1.00000000}.{b}"), "Headset Microphone (2- USB Aud", None),
            // no endpoint ID: nothing to save it by, not listed
            found(None, "Line In (Legacy)", Some("Line In (Legacy Driver)")),
            // no name at all: its ID
            found(Some("{0.0.1.00000000}.{c}"), " ", Some("")),
            found(Some("{0.0.1.00000000}.{d}"), "Микрофон (Arctis 7 Chat)", Some("Микрофон (SteelSeries Arctis 7 Chat)")),
        ];
        let mut room = [Microphone::EMPTY; 8];
        let count = list(&windows, &mut room);
        assert_eq!(
            names(&room, count),
            [
                ("{0.0.1.00000000}.{a}".into(), "Microphone (Realtek(R) Audio) Front Panel".into()),
                ("{0.0.1.00000000}.{b}".into(), "Headset Microphone (2- USB Aud".into()),
                ("{0.0.1.00000000}.{c}".into(), "{0.0.1.00000000}.{c}".into()),
                ("{0.0.1.00000000}.{d}".into(), "Микрофон (SteelSeries Arctis 7 Chat)".into()),
            ]
        );
        // the room is all there is: the first ones
        let mut small = [Microphone::EMPTY; 2];
        assert_eq!(list(&windows, &mut small), 2);
        assert_eq!(names(&small, 2), names(&room, 2));
        assert_eq!(list(&[], &mut room), 0);
        assert_eq!(list(&windows, &mut []), 0);
    }

    #[test]
    fn the_chosen_device_is_found_by_its_id_else_the_default() {
        let (headset, webcam, desk) = (id("{0.0.1.00000000}.{headset}"), id("{0.0.1.00000000}.{webcam}"), id("{0.0.1.00000000}.{desk}"));
        let plugged = [Some(desk), None, Some(headset), Some(webcam)];
        assert_eq!(device(&headset, &plugged), Device::Number(2));
        // another device unplugged: WinMM's numbers move, the choice follows its ID
        assert_eq!(device(&headset, &[Some(headset), Some(webcam)]), Device::Number(0));
        // unplugged: the default, and the choice kept for when it is back
        assert_eq!(device(&headset, &[Some(desk), Some(webcam)]), Device::Default);
        assert_eq!(device(&headset, &[]), Device::Default);
        // the same ID whatever the case of its letters
        assert_eq!(device(&id("{0.0.1.00000000}.{HEADSET}"), &plugged), Device::Number(2));
    }
}
