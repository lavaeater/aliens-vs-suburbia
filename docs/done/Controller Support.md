Controller support isn't "done" by any stretch of the imagination, but it is *"almost"* done, as we can react to its inputs. What we need to do to make it a merge-able feature is capture a button press for the option button (whatever that is in Bevy terms) and then just add a gamepadcontrol-component to the player entity.

Perhaps we might even replace the keyboard-component with a gamepad component so we don't actually support multicontrol off the bat.

# TODO
- [ ] Add gamepadcontrol
- [ ] Remove keyboardcontrol