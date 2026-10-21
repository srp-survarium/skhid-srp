package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.geom.Point;
   import flash.utils.Dictionary;
   
   public class DamageIndicator extends MovieClip
   {
      
      protected var _framesCount:uint;
      
      public var damageInd:MovieClip;
      
      protected var _hitmarks_dir:Dictionary = new Dictionary(true);
      
      protected var _hitmarks_orig:Dictionary = new Dictionary(true);
      
      protected var _hitmark_radius:Number = 230;
      
      protected var _direction:Number = 0;
      
      protected var _hitmarkContainer:Sprite;
      
      protected var _criticalInd:MovieClip;
      
      public function DamageIndicator()
      {
         super();
         _framesCount = this.totalFrames;
         this.damageInd.visible = false;
      }
      
      public function setCriticalIndicator(value:MovieClip) : void
      {
         _criticalInd = value;
      }
      
      public function setHitmarkContainer(value:Sprite) : void
      {
         _hitmarkContainer = value;
      }
      
      public function setHP(vv:Number) : void
      {
         var frame:Number = Math.ceil(this._framesCount * (vv / 100));
         this.damageInd.gotoAndStop(frame);
      }
      
      public function reset() : void
      {
         var st:Object;
         _hitmarkContainer = null;
         for(st in this._hitmarks_dir)
         {
            st.stop();
            if(st.parent)
            {
               st.parent.removeChild(st);
            }
            delete this._hitmarks_dir[st];
            delete this._hitmarks_orig[st];
            continue;
         }
      }
      
      public function hit(dir:Point, amount:Number, direct:Number = 0) : void
      {
         var bm2:MovieClip = new DirectionHitMark();
         var centre:Point = this.screenCentre();
         var px:Number = centre.x;
         var py:Number = centre.y;
         var x:Number = px + dir.x * this._hitmark_radius;
         var y:Number = py + -dir.y * this._hitmark_radius;
         bm2.x = x;
         bm2.y = y;
         this._hitmarks_dir[bm2] = this._direction;
         this._hitmarks_orig[bm2] = direct;
         var dir3:Number = this.point_direction(px,-py,x,-y) + Math.PI / 2;
         bm2.rotation = dir3 * (180 / Math.PI);
         this._hitmarkContainer.addChild(bm2);
      }
      
      public function updateHitmarks(value:Number) : void
      {
         var st:Object;
         var dir:Number;
         var dir_orig:Number;
         var new_direct:Number;
         var point:Point;
         var px:Number;
         var py:Number;
         var nx:Number;
         var ny:Number;
         var dir3:Number;
         _direction = value;
         var centre:Point = null;
         for(st in this._hitmarks_dir)
         {
            if(!st || !st.parent)
            {
               delete this._hitmarks_dir[st];
               delete this._hitmarks_orig[st];
            }
            else
            {
               dir = (this._hitmarks_dir[st]);
               dir_orig = (this._hitmarks_orig[st]);
               new_direct = dir - this._direction + dir_orig;
               point = new Point(Math.cos(new_direct),Math.sin(new_direct));
               if(centre == null)
               {
                  centre = this.screenCentre();
               }
               px = centre.x;
               py = centre.y;
               nx = px + point.x * this._hitmark_radius;
               ny = py + -point.y * this._hitmark_radius;
               st.x = nx;
               st.y = ny;
               dir3 = this.point_direction(px,-py,nx,-ny) + Math.PI / 2;
               st.rotation = dir3 * (180 / Math.PI);
            }
         }
      }
      
      // The screen's centre where the marks are drawn. Their container is a
      // child of the HUD, which scales itself from 1920 x 1080 (2 at 4K), so
      // the stage's half size in pixels would be the bottom right corner
      // there; its own coordinates of the centre are right at any scale.
      protected function screenCentre() : Point
      {
         return this._hitmarkContainer.globalToLocal(new Point(stage.stageWidth / 2,stage.stageHeight / 2));
      }

      public function crit(part_id:uint, value:uint) : void
      {
         this._criticalInd.crit(part_id,value);
      }
      
      public function partHP(part_id:uint, value:Number, breakPoint:Number) : void
      {
         this._criticalInd.partHP(part_id,value,breakPoint);
      }
      
      public function heal(part_id:uint) : void
      {
         this._criticalInd.heal(part_id);
      }
      
      public function setRadiation(vv:Number) : void
      {
         this._criticalInd.setRadiation(vv);
      }
      
      public function setToxic(vv:Number) : void
      {
         this._criticalInd.setToxic(vv);
      }
      
      public function setBleeding(vv:Number) : void
      {
         this._criticalInd.setBleeding(vv);
      }
      
      protected function point_direction(x1:Number, y1:Number, x2:Number, y2:Number) : Number
      {
         return Math.atan2(y1 - y2,x2 - x1);
      }
   }
}

