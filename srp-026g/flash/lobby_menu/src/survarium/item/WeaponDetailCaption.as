package survarium.item
{
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import temp_test_fla.weapon_compare_mc_8;

   // Captions have their own layout; they are never numeric stat rows.
   // Retain a native timeline field so Scaleform resolves the embedded font.
   public final class WeaponDetailCaption extends Sprite
   {
      public var textField:TextField;
      public var kind:String;
      public var contentHeight:Number;

      public function WeaponDetailCaption(entry:Object)
      {
         kind = entry.kind;
         var skin:weapon_compare_mc_8 = new weapon_compare_mc_8();
         skin.indicator.validateNow();
         textField = skin.indicator.textFieldLabel;
         addChild(textField);
         scaleX = skin.indicator.scaleX;
         scaleY = skin.indicator.scaleY;
         x = skin.indicator.x;
         y = skin.indicator.y;
         var format:TextFormat = textField.getTextFormat();
         format.size = kind == "heading" ? 16 : 14;
         format.color = kind == "heading" ? 0xD7DEDC : kind == "hint" ? 0x7B8581 : 0xBCCCCB;
         textField.defaultTextFormat = format;
         textField.multiline = textField.wordWrap = true;
         textField.width = skin.indicator.textFieldValue.x + skin.indicator.textFieldValue.width - textField.x;
         textField.autoSize = TextFieldAutoSize.LEFT;
         textField.text = entry.text;
         textField.setTextFormat(format);
         textField.selectable = false;
         mouseEnabled = mouseChildren = false;
         contentHeight = textField.textHeight + (kind == "heading" ? 12 : 7);
         if(kind == "heading")
         {
            graphics.beginFill(0x080A09);
            graphics.drawRect(textField.x - 3,textField.y - 3,textField.width + 6,contentHeight);
            graphics.endFill();
            graphics.lineStyle(1,0x2D3431);
            graphics.moveTo(textField.x - 3,textField.y - 3);
            graphics.lineTo(textField.x + textField.width + 3,textField.y - 3);
         }
      }
   }
}
