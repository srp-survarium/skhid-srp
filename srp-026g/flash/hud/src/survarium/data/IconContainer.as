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
         if(count)
         {
            times = count;
         }
         var baseTexture:BitmapData;
         baseTexture = iconBitmap;
         function create_icon(c:int, r:int, atStart:Boolean):void
         {
            var iconBmpData:BitmapData = new BitmapData(iconW,iconH,true,0);
            var mat:Matrix = new Matrix();
            if(atStart)
            {
               mat.translate(-(iconW + stepH) * c - stepH,-(iconH + stepV) * r - stepV);
            }
            else
            {
               mat.translate(-(iconW + stepH) * c,-(iconH + stepV) * r);
            }
            iconBmpData.draw(baseTexture,mat,null,null,new Rectangle(0,0,iconW,iconH));
            _icons.push(iconBmpData);
         }
         var r:uint;
         var c:uint;
         columns = Math.floor(baseTexture.width / (iconW + stepH));
         rows = Math.floor(baseTexture.height / (iconH + stepV));
         if(!vertical)
         {
            for(r = 0; r < this.rows; r++)
            {
               for(c = 0; c < this.columns; c++)
               {
                  create_icon(c,r,atStart);
                  if(--times == 0)
                  {
                     return;
                  }
               }
            }
         }
         else
         {
            for(c = 0; c < this.columns; c++)
            {
               for(r = 0; r < this.rows; r++)
               {
                  create_icon(c,r,atStart);
                  if(--times == 0)
                  {
                     return;
                  }
               }
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

