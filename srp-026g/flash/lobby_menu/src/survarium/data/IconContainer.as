package survarium.data
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.PixelSnapping;
   import flash.geom.Matrix;
   import flash.geom.Rectangle;
   import flash.utils.Dictionary;
   import survarium.core.Icon;
   
   public class IconContainer
   {
      
      protected static var _ids:Dictionary = new Dictionary();
      
      public var rows:uint = 0;
      
      public var columns:uint = 0;
      
      protected var _icons:Array = new Array();
      
      public function IconContainer(name:String, iconW:uint, iconH:uint, iconBitmap:BitmapData, vertical:Boolean = false, count:uint = 0, stepH:uint = 0, stepV:uint = 0, atStart:Boolean = true)
      {
         _ids[name] = this;
         var times:int = -1;
         var baseTexture:BitmapData;
         function create_icon(param1:int, param2:int, param3:Boolean):void
         {
            var _loc4_:BitmapData = new BitmapData(iconW,iconH,true,0);
            var _loc5_:Matrix = new Matrix();
            if(param3)
            {
               _loc5_.translate(-(iconW + stepH) * param1 - stepH,-(iconH + stepV) * param2 - stepV);
            }
            else
            {
               _loc5_.translate(-(iconW + stepH) * param1,-(iconH + stepV) * param2);
            }
            _loc4_.draw(baseTexture,_loc5_,null,null,new Rectangle(0,0,iconW,iconH));
            _icons.push(_loc4_);
         }
         var r:uint;
         var c:uint;
         if(count)
         {
            times = count;
         }
         baseTexture = iconBitmap;
         columns = Math.floor(baseTexture.width / (iconW + stepH));
         rows = Math.floor(baseTexture.height / (iconH + stepV));
         if(!vertical)
         {
            r = 0;
            while(r < this.rows)
            {
               c = 0;
               while(c < this.columns)
               {
                  create_icon(c,r,atStart);
                  if(--times == 0)
                  {
                     return;
                  }
                  c++;
               }
               r++;
            }
         }
         else
         {
            c = 0;
            while(c < this.columns)
            {
               r = 0;
               while(r < this.rows)
               {
                  create_icon(c,r,atStart);
                  if(--times == 0)
                  {
                     return;
                  }
                  r++;
               }
               c++;
            }
         }
         trace("IconCCreated::",name,this._icons.length);
      }
      
      public static function getInstance(param1:String) : IconContainer
      {
         return _ids[param1];
      }
      
      public function getIcon(param1:uint) : Icon
      {
         var _loc2_:Bitmap = new Bitmap(this._icons[param1],PixelSnapping.ALWAYS);
         _loc2_.smoothing = true;
         var _loc3_:Icon = new Icon();
         _loc3_.loadImage(_loc2_);
         return _loc3_;
      }
      
      public function toString() : String
      {
         return "[IconContainer]";
      }
   }
}

