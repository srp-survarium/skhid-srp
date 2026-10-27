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
   // native countdown labels below that row, and restore their original
   // geometry and styling when the same HUD switches to another mode.
   public class ResearchCountdownLayout
   {
      public var respawn:Label;
      private var _pregame:MovieClip;
      private var _texts:Array;
      private var _respawnPosition:Point;
      private var _pregamePosition:Point;
      private var _alignH:String;
      private var _alignV:String;

      public function ResearchCountdownLayout(respawn:Label, pregame:MovieClip)
      {
         this.respawn = respawn;
         this._pregame = pregame;
         respawn.validateNow();
         this._texts = [save(respawn.textField),save(pregame.contextText)];
         this._respawnPosition = new Point(respawn.x,respawn.y);
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
         respawn.validateNow();
         style(respawn.textField);
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
         bounds = respawn.textField.getBounds(respawn);
         respawn.x = Math.round(anchor.x - bounds.x - bounds.width / 2);
         respawn.y = Math.round(anchor.y - bounds.y);
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
         respawn.x = this._respawnPosition.x;
         respawn.y = this._respawnPosition.y;
         this._pregame.x = this._pregamePosition.x;
         this._pregame.y = this._pregamePosition.y;
         this._pregame.layoutData.alignH = this._alignH;
         this._pregame.layoutData.alignV = this._alignV;
      }
   }
}
