package survarium.rooms
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.utils.getDefinitionByName;
   import survarium.controls.Modals;

   // A window of cards to pick one from, in the rooms' map-vote look (RoomsLook, RoomsRoom): the
   // lobby window (LobbyWindow) and the faces' window (FaceWindow). A title over the cards in rows
   // (a shorter last row centred); a card is its picture over the whole card and its name in white
   // on a band of black at 0.62 along its bottom; the marked card framed gold (3 px), the others
   // 1 px, as a vote card is. A press on a card is `pressed(card)`, the card's index: given to the
   // card itself (its click) or to the window, which Scaleform does, and RoomsLook.route takes it
   // to the card under it, as in the rooms window.
   //
   // Its button toggles it (`toggle`): opened in the middle of the lobby, a modal (Esc closes it,
   // Modals); `close` takes it down.
   public class CardWindow extends Sprite
   {
      private static const GAP:int = 14;

      private static const PAD:int = 16;

      private static const TITLE_H:int = 44;

      // The name's line, from the band's top.
      private static const NAME_TOP:int = 4;

      private var _font:String;

      private var _bold:Object;

      private var _cardW:int;

      private var _cardH:int;

      private var _bandH:int;

      private var _nameSize:int;

      // Made again with the cards: its routes are theirs.
      private var _look:RoomsLook;

      private var _title:TextField;

      private var _titleText:String = "";

      private var _frames:Array = [];

      private var _names:Array = [];

      // The card framed gold (-1: none).
      private var _marked:int = -1;

      public function CardWindow(cardW:int, cardH:int, bandH:int, nameSize:int)
      {
         super();
         var font:TextFormat = gameFont();
         this._font = font.font;
         this._bold = font.bold;
         this._cardW = cardW;
         this._cardH = cardH;
         this._bandH = bandH;
         this._nameSize = nameSize;
         addEventListener(MouseEvent.MOUSE_DOWN,this.onDown);
      }

      // The font of the bottom bar's tabs (the game's own), as the rooms window's.
      public static function gameFont() : TextFormat
      {
         var tabClass:Class = getDefinitionByName("ButtonChatTabBottom") as Class;
         var sample:* = new tabClass();
         var tabText:TextField = sample["textField"] as TextField;
         return tabText != null ? tabText.getTextFormat() : new TextFormat();
      }

      // The cards, made again: a picture each (cardW x cardH), in rows of `columns`.
      protected function build(pictures:Array, columns:int) : void
      {
         while(numChildren > 0)
         {
            removeChildAt(0);
         }
         this._frames = [];
         this._names = [];
         this._look = new RoomsLook(this,this,this._font,this._bold);
         var count:int = pictures.length;
         columns = Math.max(1,columns);
         var rows:int = Math.max(1,Math.ceil(count / columns));
         var w:Number = PAD * 2 + this._cardW * columns + GAP * (columns - 1);
         var h:Number = TITLE_H + this._cardH * rows + GAP * (rows - 1) + PAD;
         graphics.clear();
         graphics.beginFill(0x0E1214,0.96);
         graphics.drawRect(0,0,w,h);
         graphics.endFill();
         graphics.lineStyle(1,0x3A4248);
         graphics.drawRect(0,0,w,h);
         this._title = this._look.text(this,this._titleText,PAD,14,w - PAD * 2,16,RoomsLook.LIGHT);
         for(var i:int = 0; i < count; i++)
         {
            var row:int = int(i / columns);
            var inRow:int = Math.min(columns,count - row * columns);
            var cell:Sprite = new Sprite();
            cell.x = PAD + Math.round((columns - inRow) * (this._cardW + GAP) / 2) + i % columns * (this._cardW + GAP);
            cell.y = TITLE_H + row * (this._cardH + GAP);
            cell.addChild(pictures[i] as DisplayObject);
            var shade:Shape = new Shape();
            // the whole card hit (Scaleform does not hit-test alpha 0), the band
            shade.graphics.beginFill(0,0.01);
            shade.graphics.drawRect(0,0,this._cardW,this._cardH);
            shade.graphics.endFill();
            shade.graphics.beginFill(0,0.62);
            shade.graphics.drawRect(0,this._cardH - this._bandH,this._cardW,this._bandH);
            shade.graphics.endFill();
            cell.addChild(shade);
            var frame:Shape = new Shape();
            cell.addChild(frame);
            this._frames.push(frame);
            this._names.push(this._look.text(cell,"",12,this._cardH - this._bandH + NAME_TOP,this._cardW - 24,this._nameSize,0xFFFFFF));
            cell.buttonMode = true;
            cell.mouseChildren = false;
            cell.name = String(i);
            cell.addEventListener(MouseEvent.CLICK,this.onCard,false,0,true);
            this._look.route(cell,this.onRouted);
            addChild(cell);
         }
         this.redraw();
      }

      public function get count() : int
      {
         return this._frames.length;
      }

      protected function setTitle(text:String) : void
      {
         this._titleText = text;
         if(this._title != null)
         {
            this._title.text = text;
         }
      }

      protected function setLabel(card:int, text:String) : void
      {
         if(card >= 0 && card < this._names.length)
         {
            TextField(this._names[card]).text = text;
         }
      }

      // The card framed gold (-1: none).
      protected function mark(card:int) : void
      {
         this._marked = card;
         this.redraw();
      }

      private function redraw() : void
      {
         for(var i:int = 0; i < this._frames.length; i++)
         {
            var mine:Boolean = i == this._marked;
            var frame:Shape = this._frames[i];
            frame.graphics.clear();
            frame.graphics.lineStyle(mine ? 3 : 1,mine ? RoomsLook.GOLD : 0x26323C);
            frame.graphics.drawRect(0,0,this._cardW,this._cardH);
         }
      }

      // A card pressed: what the window does with it.
      protected function pressed(card:int) : void
      {
      }

      public function get isOpen() : Boolean
      {
         return parent != null;
      }

      // Its button's click: opened in the middle of `host` (the lobby), a modal; closed when open.
      public function toggle(host:DisplayObjectContainer) : void
      {
         if(this.isOpen)
         {
            this.close();
            return;
         }
         if(host.stage != null)
         {
            x = int((host.stage.stageWidth / host.scaleX - width) / 2);
            y = int((host.stage.stageHeight / host.scaleY - height) / 2);
         }
         host.addChild(this);
         Modals.open(this,Modals.LOBBY,this.close,this.shown);
      }

      public function close() : void
      {
         if(parent != null)
         {
            parent.removeChild(this);
         }
         Modals.leave(this);
      }

      private function shown() : Boolean
      {
         return parent != null;
      }

      private function onDown(e:MouseEvent) : void
      {
         if(this._look != null)
         {
            this._look.onContentDown(e);
         }
      }

      private function onCard(e:Event) : void
      {
         this.press(e.currentTarget as DisplayObject);
      }

      private function onRouted(control:DisplayObject, e:MouseEvent) : void
      {
         this.press(control);
      }

      private function press(card:DisplayObject) : void
      {
         RoomsLook.clickSound();
         this.pressed(int(card.name));
      }
   }
}
