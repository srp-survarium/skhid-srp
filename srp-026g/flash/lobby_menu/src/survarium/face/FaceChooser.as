package survarium.face
{
   import flash.display.DisplayObject;
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import scaleform.gfx.MouseEventEx;
   import survarium.data.IconContainer;
   import survarium.rooms.RoomsLook;

   // The faces to choose from (face-choice.md, Lobby), opened by the Face button under it: a tile a
   // face, as 0.69's chooser (MessageEnterName's face list), the account's face framed gold as the
   // lobby window's pick is. A press on a tile chooses that face at once (FaceChoice.pick); the
   // lobby's character shows it when srp's account comes. It stays open to try another; a press
   // outside or Esc closes it (a transient window, the button's).
   //
   // A face's picture is the lobby's equipment atlas cell the faces pack draws for it (its
   // heads[i].icon, faces-pack.md: row i + 1 of ICON_COLUMN); an atlas without that cell shows
   // the face's number instead.
   public class FaceChooser extends Sprite
   {

      // The atlas's column of the faces (resource-porter's faces_026::ICON_COLUMN), from 1.
      public static const ICON_COLUMN:int = 16;

      // An equipment cell (171 x 83) shows its middle in a square tile.
      public static const TILE:int = 88;

      private static const ICON_W:int = 171;

      private static const ICON_H:int = 83;

      // The most tiles a row: more faces make rows of as many as even them out.
      private static const ROW_MOST:int = 6;

      private static const GAP:int = 6;

      private static const PAD:int = 10;

      private static const NUMBER_SIZE:int = 28;

      // As the presets' rows: hover lightens a tile.
      private static const HOVER_ALPHA:Number = 0.08;

      private var _choice:FaceChoice;

      // a Sprite a face, in face order, and the frame drawn over each
      private var _tiles:Array = [];

      private var _frames:Array = [];

      // The tile under the pointer (-1: none).
      private var _over:int = -1;

      public function FaceChooser(choice:FaceChoice)
      {
         super();
         name = "faceChooser";
         this._choice = choice;
         this._choice.addEventListener(Event.CHANGE,this.onChoice);
         addEventListener(MouseEvent.MOUSE_DOWN,this.onPress);
         addEventListener(MouseEvent.MOUSE_MOVE,this.onMove);
         addEventListener(MouseEvent.ROLL_OUT,this.onOut);
         this.build();
      }

      // The tiles in rows of `columns`.
      public static function columnsOf(count:int) : int
      {
         var rows:int = Math.ceil(count / ROW_MOST);
         return Math.max(1,Math.ceil(count / Math.max(1,rows)));
      }

      // The face of a tile pressed at a stage point (-1: none).
      public function faceAt(stageX:Number, stageY:Number) : int
      {
         for(var i:int = 0; i < this._tiles.length; i++)
         {
            if(DisplayObject(this._tiles[i]).hitTestPoint(stageX,stageY,false))
            {
               return i;
            }
         }
         return -1;
      }

      // The tiles made again when the count of faces changed; the marks drawn otherwise.
      private function onChoice(e:Event) : void
      {
         if(this._choice.count != this._tiles.length)
         {
            this.build();
         }
         else
         {
            this.mark();
         }
      }

      private function build() : void
      {
         while(numChildren > 0)
         {
            removeChildAt(0);
         }
         this._tiles = [];
         this._frames = [];
         this._over = -1;
         var count:int = this._choice.count;
         var columns:int = columnsOf(count);
         var rows:int = Math.ceil(count / columns);
         var w:Number = PAD * 2 + columns * TILE + (columns - 1) * GAP;
         var h:Number = PAD * 2 + rows * TILE + (rows - 1) * GAP;
         var g:Graphics = graphics;
         g.clear();
         // the lobby window's back and border (LobbyWindow)
         g.beginFill(0x3A4248,1);
         g.drawRect(0,0,w,h);
         g.endFill();
         g.beginFill(0x0E1214,0.97);
         g.drawRect(1,1,w - 2,h - 2);
         g.endFill();
         for(var i:int = 0; i < count; i++)
         {
            var tile:Sprite = new Sprite();
            tile.name = "face" + i;
            tile.x = PAD + i % columns * (TILE + GAP);
            tile.y = PAD + int(i / columns) * (TILE + GAP);
            tile.buttonMode = true;
            tile.mouseChildren = false;
            // Alpha 0 is not hit-tested in Scaleform; the cover's dark under a clear picture.
            tile.graphics.beginFill(0x05080B,1);
            tile.graphics.drawRect(0,0,TILE,TILE);
            tile.graphics.endFill();
            tile.addChild(this.picture(i));
            var frame:Shape = new Shape();
            tile.addChild(frame);
            addChild(tile);
            this._tiles.push(tile);
            this._frames.push(frame);
         }
         this.mark();
      }

      // The face's atlas cell, its middle in the tile; its number without one.
      private function picture(face:int) : DisplayObject
      {
         var icons:IconContainer = IconContainer.getInstance("equipment");
         if(icons != null && icons.columns >= ICON_COLUMN && face < icons.rows)
         {
            var icon:DisplayObject = icons.getIcon((ICON_COLUMN - 1) * icons.rows + face);
            icon.x = Math.round((TILE - ICON_W) / 2);
            icon.y = Math.round((TILE - ICON_H) / 2);
            var mask:Shape = new Shape();
            mask.graphics.beginFill(0,1);
            mask.graphics.drawRect(0,0,TILE,TILE);
            mask.graphics.endFill();
            var box:Sprite = new Sprite();
            box.addChild(icon);
            box.addChild(mask);
            icon.mask = mask;
            return box;
         }
         var number:TextField = new TextField();
         number.defaultTextFormat = new TextFormat("Calibri",NUMBER_SIZE,RoomsLook.GREY,true);
         number.embedFonts = true;
         number.selectable = false;
         number.mouseEnabled = false;
         number.multiline = false;
         number.wordWrap = false;
         number.autoSize = TextFieldAutoSize.LEFT;
         number.text = String(face + 1);
         number.x = Math.round((TILE - number.width) / 2);
         number.y = Math.round((TILE - number.height) / 2);
         return number;
      }

      // The account's face framed gold (3 px), the others as a vote card is (1 px); the tile under
      // the pointer lightened.
      private function mark() : void
      {
         for(var i:int = 0; i < this._frames.length; i++)
         {
            var mine:Boolean = i == this._choice.shown;
            var g:Graphics = Shape(this._frames[i]).graphics;
            var line:int = mine ? 3 : 1;
            g.clear();
            if(i == this._over)
            {
               g.beginFill(0xFFFFFF,HOVER_ALPHA);
               g.drawRect(0,0,TILE,TILE);
               g.endFill();
            }
            // the frame as four filled strips inside the tile (no pen: no stray line)
            g.beginFill(mine ? RoomsLook.GOLD : 0x26323C,1);
            g.drawRect(0,0,TILE,line);
            g.drawRect(0,TILE - line,TILE,line);
            g.drawRect(0,line,line,TILE - line * 2);
            g.drawRect(TILE - line,line,line,TILE - line * 2);
            g.endFill();
         }
      }

      // A press on a tile (given to it, or to the chooser itself: scaleform-movie-pitfalls; one
      // given to the transient layer's holder comes here too, Transient.onHolderPress).
      private function onPress(e:MouseEvent) : void
      {
         var ex:MouseEventEx = e as MouseEventEx;
         if(ex != null && ex.buttonIdx != MouseEventEx.LEFT_BUTTON)
         {
            return;
         }
         var face:int = this.faceAt(e.stageX,e.stageY);
         if(face < 0)
         {
            return;
         }
         RoomsLook.clickSound();
         this._choice.pick(face);
      }

      private function onMove(e:MouseEvent) : void
      {
         var face:int = this.faceAt(e.stageX,e.stageY);
         if(face != this._over)
         {
            this._over = face;
            this.mark();
         }
      }

      private function onOut(e:MouseEvent) : void
      {
         if(this._over >= 0)
         {
            this._over = -1;
            this.mark();
         }
      }
   }
}
