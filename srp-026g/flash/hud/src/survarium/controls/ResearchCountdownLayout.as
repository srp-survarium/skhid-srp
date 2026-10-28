package survarium.controls
{
   import flash.display.DisplayObjectContainer;
   import flash.display.MovieClip;
   import flash.filters.GlowFilter;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import scaleform.clik.controls.Label;

   // Research alone adds a row of capture points below the score. Keep the
   // countdown labels below that row. The respawn label's CLIK sizing and
   // timeline must not resize our text during the first frame of a death.
   public class ResearchCountdownLayout
   {
      public var respawn:Label;
      private var _pregame:MovieClip;
      private var _texts:Array;
      private var _respawnText:TextField;
      private var _respawnVisible:Boolean;
      private var _pregamePosition:Point;
      private var _alignH:String;
      private var _alignV:String;

      public function ResearchCountdownLayout(respawn:Label, pregame:MovieClip)
      {
         this.respawn = respawn;
         this._pregame = pregame;
         respawn.validateNow();
         this._texts = [save(pregame.contextText)];
         this._respawnVisible = respawn.visible;
         this._respawnText = new TextField();
         this._respawnText.defaultTextFormat = respawn.textField.defaultTextFormat;
         this._respawnText.embedFonts = respawn.textField.embedFonts;
         this._respawnText.selectable = false;
         this._respawnText.mouseEnabled = false;
         this._respawnText.multiline = false;
         style(this._respawnText);
         this._respawnText.visible = false;
         respawn.parent.addChild(this._respawnText);
         respawn.visible = false;
         this._pregamePosition = new Point(pregame.x,pregame.y);
         this._alignH = pregame.layoutData.alignH;
         this._alignV = pregame.layoutData.alignV;
         pregame.layoutData.alignH = pregame.layoutData.alignV = "none";
      }

      private static function save(field:TextField) : Object
      {
         return {field:field,format:field.defaultTextFormat,filters:field.filters,
            autoSize:field.autoSize,wrap:field.wordWrap,
            x:field.x,y:field.y,width:field.width,height:field.height};
      }

      private static function style(field:TextField) : void
      {
         var format:TextFormat = field.defaultTextFormat;
         format.size = 18;
         format.color = 0xFFFFFF;
         format.align = "center";
         field.defaultTextFormat = format;
         field.setTextFormat(format);
         field.wordWrap = false;
         field.autoSize = TextFieldAutoSize.CENTER;
         field.filters = [new GlowFilter(0,0.9,3,3,3)];
      }

      public function place(units:ResearchUnits) : void
      {
         var hud:DisplayObjectContainer = units.parent;
         if(hud == null || respawn.parent == null)
         {
            return;
         }
         // Read the engine's value, not the asynchronously validated textField.
         // This sibling inherits the score indicator's visibility/transform,
         // but is unaffected by the native label's animation or constraints.
         respawn.visible = false;
         var text:String = respawn.text == null ? "" : respawn.text;
         if(this._respawnText.text != text)
         {
            this._respawnText.text = text;
         }
         this._respawnText.visible = text.length > 0;
         style(this._pregame.contextText);
         var bottom:Number = hud.globalToLocal(units.localToGlobal(new Point(0,units.contentBottom))).y + 4;
         var bounds:Rectangle = this._pregame.contextText.getBounds(this._pregame);
         this._pregame.x = Math.round(units.x - bounds.x - bounds.width / 2);
         this._pregame.y = Math.round(bottom - bounds.y);
         // If the engine sends both labels during a transition, use two
         // rows instead of drawing them on top of one another.
         if(this._pregame.visible && this._pregame.contextText.length > 0)
         {
            bottom += bounds.height + 6;
         }
         var anchor:Point = respawn.parent.globalToLocal(hud.localToGlobal(new Point(units.x,bottom)));
         this._respawnText.x = Math.round(anchor.x - this._respawnText.width / 2);
         this._respawnText.y = Math.round(anchor.y);
      }

      public function restore() : void
      {
         for each(var saved:Object in this._texts)
         {
            var field:TextField = saved.field;
            field.autoSize = TextFieldAutoSize.NONE;
            field.wordWrap = saved.wrap;
            field.defaultTextFormat = saved.format;
            field.setTextFormat(saved.format);
            field.filters = saved.filters;
            field.width = saved.width;
            field.height = saved.height;
            field.x = saved.x;
            field.y = saved.y;
            field.autoSize = saved.autoSize;
         }
         if(this._respawnText.parent != null)
         {
            this._respawnText.parent.removeChild(this._respawnText);
         }
         respawn.visible = this._respawnVisible;
         this._pregame.x = this._pregamePosition.x;
         this._pregame.y = this._pregamePosition.y;
         this._pregame.layoutData.alignH = this._alignH;
         this._pregame.layoutData.alignV = this._alignV;
      }
   }
}
