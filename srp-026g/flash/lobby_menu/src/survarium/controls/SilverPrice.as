package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.Sprite;
   import flash.text.TextFieldAutoSize;
   import survarium.buttons.CheckBoxValue;

   // A window's price in silver, centred, in place of the stock money choice: the game has one currency,
   // so a radio box choosing it means nothing (the owner, 2026-11-08: "it shouldn't be a pin button", "it
   // should just be centered silver price"). It takes the place of the window's silver CheckBoxValue,
   // which it hides for good; the game's own price label (SrvCostLabelShared, the buy dialog's) shows it.
   public class SilverPrice extends Sprite
   {

      // The gap above and below a window's price row, and what a window's bottom keeps under its buttons'
      // top (MessageCraftItem/MessageRerollItem.updateLayout: the buttons' top is the bottom less 47).
      public static const GAP:Number = 14;

      public static const BUTTON_AREA:Number = 47;

      private var _label:SrvCostLabelShared = new SrvCostLabelShared();

      private var _stood:CheckBoxValue;

      private var _centreX:Number;

      // `stood`: the checkbox it replaces (its height in the window is the price's); `centreX`: the
      // window's centre, in the checkbox's parent's coordinates.
      public function SilverPrice(stood:CheckBoxValue, centreX:Number)
      {
         super();
         this._stood = stood;
         this._centreX = centreX;
         this._label.moveIcon = true;
         this._label.textField.autoSize = TextFieldAutoSize.LEFT;
         addChild(this._label);
         stood.visible = false;
         stood.parent.addChildAt(this,stood.parent.getChildIndex(stood));
         this.cost = 0;
      }

      // The window height that leaves GAP between the price row and the buttons.
      public function get windowHeight() : Number
      {
         return this.y + this._label.textField.y + this._label.textField.height + GAP + BUTTON_AREA;
      }

      // For a window whose buttons the timeline (or its own code) placed: the buttons move to GAP under
      // the price row, and the window's bottom keeps its distance under them. The window's height changes
      // with it (`background`); the buttons share a row (the first one's y).
      public function fitWindow(background:DisplayObject, buttons:Array) : void
      {
         var underButtons:Number = background.height - DisplayObject(buttons[0]).y;
         var top:Number = this.windowHeight - BUTTON_AREA;
         for each(var button:DisplayObject in buttons)
         {
            button.y = top;
         }
         background.height = top + underButtons;
      }

      public function set cost(value:uint) : void
      {
         this._label.cost = value;
         // The icon left of the text: centre the two together, and put the text's top where the
         // checkbox had its value's (the checkbox may have moved since: craft's closed-up rows).
         var left:Number = this._label.icon.x;
         var right:Number = this._label.textField.x + this._label.textField.width;
         this._label.x = -(left + right) / 2;
         this.x = this._centreX;
         var stoodValue:DisplayObject = this._stood.textFieldValue;
         this.y = this._stood.y + (stoodValue ? stoodValue.y : 0) - this._label.textField.y;
      }
   }
}
