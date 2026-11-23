package survarium.menu.selector
{
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.text.TextField;
   
   // An action to pick, drawn as the Settings' own key row (SrvBindKeyListRender: its key box with the
   // action's key now, its label), shown only: the row takes no press of its own (its key box would ask
   // the exe for a key). Its kind's colour a thin mark left of the key box; a lobby action's label
   // followed by "Lobby".
   public class ActionRow extends SelectorRow
   {
      
      public static const HEIGHT:Number = 32;
      
      // the stock row's place, and its key box widened for the mouse's and the modifiers' names
      private static const STOCK_X:Number = 12;
      
      public static const KEY_BOX:Number = 96;
      
      private static const LABEL_GAP:Number = 15;
      
      public var action:BoundAction;
      
      public var stock:MovieClip;
      
      public var label:TextField;
      
      public function ActionRow(action:BoundAction, texts:SelectorTexts, rowWidth:Number)
      {
         super(rowWidth,HEIGHT);
         this.action = action;
         var g:Graphics = graphics;
         g.beginFill(action.colour,0.85);
         g.drawRect(5,8,3,HEIGHT - 16);
         g.endFill();
         this.stock = new SrvBindKeyListRender();
         this.stock.mouseEnabled = this.stock.mouseChildren = false;
         this.stock.alpha = 1;
         this.stock.x = STOCK_X;
         this.stock.y = Math.round((HEIGHT - 29) / 2);
         this.stock.data = {label:action.label,key:action.key == null ? "" : action.key,type:1};
         addChild(this.stock);
         widen(this.stock);
         this.label = this.stock.textField;
         var room:Number = rowWidth - STOCK_X - this.label.x - 12;
         var tag:TextField = null;
         if(action.lobby)
         {
            tag = Paint.text(13,Paint.DIM,this.label.filters);
            tag.text = texts.lobby;
            room -= tag.width + 10;
         }
         this.label.width = room;
         Paint.fit(this.label,action.label,room);
         if(tag != null)
         {
            tag.x = this.stock.x + this.label.x + this.label.textWidth + 12;
            tag.y = this.stock.y + this.label.y + this.label.textHeight - tag.textHeight - 1;
            addChild(tag);
         }
      }
      
      // A stock key row (SrvBindKeyListRender) with its key box KEY_BOX wide, its label after it.
      public static function widen(stock:MovieClip) : void
      {
         stock.validateNow();
         stock.btn_key.width = KEY_BOX;
         stock.btn_key.validateNow();
         stock.textField.x = KEY_BOX + LABEL_GAP;
         if(stock.back != null)
         {
            stock.back.visible = false;
         }
      }
      
      override public function get choice() : Boolean
      {
         return true;
      }
      
      override protected function get accent() : uint
      {
         return this.action.colour;
      }
   }
}
