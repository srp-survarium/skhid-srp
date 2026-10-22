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
         function create_icon(column:int, row:int, skipLeadingStep:Boolean):void
         {
            var icon:BitmapData = new BitmapData(iconW,iconH,true,0);
            var matrix:Matrix = new Matrix();
            if(skipLeadingStep)
            {
               matrix.translate(-(iconW + stepH) * column - stepH,-(iconH + stepV) * row - stepV);
            }
            else
            {
               matrix.translate(-(iconW + stepH) * column,-(iconH + stepV) * row);
            }
            icon.draw(baseTexture,matrix,null,null,new Rectangle(0,0,iconW,iconH));
            _icons.push(icon);
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
      
      public static function getInstance(value:String) : IconContainer
      {
         return _ids[value];
      }
      
      public function getIcon(iconID:uint) : Icon
      {
         var bmp:Bitmap = new Bitmap(this._icons[iconID],PixelSnapping.ALWAYS);
         bmp.smoothing = true;
         var icon:Icon = new Icon();
         icon.loadImage(bmp);
         return icon;
      }
      
      public function toString() : String
      {
         return "[IconContainer]";
      }
   }
}

