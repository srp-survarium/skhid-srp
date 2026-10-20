package survarium.controls
{
   import flash.display.Sprite;
   
   public class IconRenderer extends Sprite
   {
      
      private var _content:Sprite = null;
      
      private var _contentCanvas:Sprite = new Sprite();
      
      private var _center:Boolean = false;
      
      public function IconRenderer()
      {
         super();
         mouseChildren = false;
         addChild(this._contentCanvas);
      }
      
      public function set center(param1:Boolean) : void
      {
         _center = param1;
      }
      
      public function get center() : Boolean
      {
         return this._center;
      }
      
      public function set content(param1:Sprite) : void
      {
         if(this._content === param1)
         {
            return;
         }
         if(this._content)
         {
            if(this._contentCanvas.contains(this._content))
            {
               this._contentCanvas.removeChild(this._content);
            }
         }
         _content = param1;
         if(this._content == null)
         {
            return;
         }
         var _loc2_:Number = Math.min(width / this._content.width,height / this._content.height,1);
         _loc2_ = Math.round(_loc2_ / 0.1) * 0.1;
         trace("IconRenderer:UpdateValue:_scale",_loc2_,this._contentCanvas,name,width,height,_loc2_ / scaleX,_loc2_ / scaleY);
         this._contentCanvas.scaleX = _loc2_ / scaleX;
         this._contentCanvas.scaleY = _loc2_ / scaleY;
         if(this._content != this)
         {
            trace("center::",this._content.width,this._content.height,width,height,"cv::",this._content.width * _loc2_,(width - this._content.width * _loc2_) / 2,(height - this._content.height * _loc2_) / 2);
            this._content.x = this._center ? (width - this._content.width * _loc2_) / 2 / this._contentCanvas.scaleX / scaleX : 0;
            this._content.y = this._center ? (height - this._content.height * _loc2_) / 2 / this._contentCanvas.scaleY / scaleY : 0;
            this._contentCanvas.addChild(this._content);
         }
      }
      
      public function get content() : Sprite
      {
         return this._content;
      }
   }
}

