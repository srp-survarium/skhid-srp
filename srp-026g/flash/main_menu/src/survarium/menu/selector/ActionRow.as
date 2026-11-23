package survarium.menu.selector
{
   import flash.display.Graphics;
   import flash.display.Sprite;
   import flash.text.TextField;

   // An action to pick: its group's dot, its label (and "Lobby" for a lobby key), its key now on the
   // right (tinted when it is the key being edited, a dash when it has none).
   public class ActionRow extends SelectorRow
   {

      public static const HEIGHT:Number = 38;

      public var action:BoundAction;

      public var label:TextField;

      public var keyChip:Sprite;

      public function ActionRow(action:BoundAction, editedKey:String, texts:SelectorTexts, rowWidth:Number)
      {
         super(rowWidth,HEIGHT);
         this.action = action;
         var g:Graphics = marks.graphics;
         g.beginFill(action.group.colour);
         g.drawCircle(19,HEIGHT / 2,4);
         g.endFill();
         var right:Number;
         if(action.key == null)
         {
            var dash:TextField = Paint.text(16,Paint.FAINT);
            dash.text = "—";
            dash.x = rowWidth - 24 - dash.width / 2;
            dash.y = Math.round((HEIGHT - dash.height) / 2);
            addChild(dash);
            right = rowWidth - 44;
         }
         else
         {
            this.keyChip = chip(action.key,action.key == editedKey ? int(action.group.colour) : -1);
            right = this.keyChip.x - 14;
         }
         var tag:Sprite = null;
         if(action.lobby)
         {
            tag = lobbyTag(texts.lobby);
            right -= tag.width + 10;
         }
         this.label = Paint.text(17,Paint.INK);
         Paint.fit(this.label,action.label,right - 34);
         this.label.x = 32;
         this.label.y = Math.round((HEIGHT - this.label.height) / 2);
         addChild(this.label);
         if(tag != null)
         {
            tag.x = this.label.x + this.label.width + 8;
            tag.y = Math.round((HEIGHT - tag.height) / 2);
            addChild(tag);
         }
      }

      override public function get choice() : Boolean
      {
         return true;
      }

      override protected function get accent() : uint
      {
         return this.action.group.colour;
      }

      private static function lobbyTag(text:String) : Sprite
      {
         var tag:Sprite = new Sprite();
         var field:TextField = Paint.text(11,Paint.DIM,1);
         field.text = text.toUpperCase();
         field.x = 6;
         tag.addChild(field);
         var g:Graphics = tag.graphics;
         g.lineStyle(1,Paint.EDGE,1,true);
         g.drawRoundRect(0.5,0.5,field.width + 11,field.height,8,8);
         return tag;
      }
   }
}
