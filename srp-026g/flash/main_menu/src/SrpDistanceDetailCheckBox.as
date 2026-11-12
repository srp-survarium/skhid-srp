package
{
    // The stock checkbox's text field is only 28px high. Translated labels
    // need two or three lines, including after a checkbox state-frame change.
    public class SrpDistanceDetailCheckBox extends CheckBoxSkinned
    {
        override protected function configUI():void
        {
            super.configUI();
            constraintsDisabled = true;
            preventAutosizing = true;
        }

        override protected function draw():void
        {
            super.draw();
            if (textField != null)
            {
                textField.width = 254;
                textField.height = Math.max(44, textField.textHeight + 4);
            }
        }
    }
}
