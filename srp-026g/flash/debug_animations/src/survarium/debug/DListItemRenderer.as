package survarium.debug
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import flash.text.TextFormat;
   
   public class DListItemRenderer extends Sprite
   {
      
      public var _progress:MovieClip;
      
      public var _back:MovieClip;
      
      public var _txback:MovieClip;
      
      public var textField:TextField;
      
      protected var bgColor:uint;
      
      protected var pgColor:uint;
      
      protected var txAlign:uint;
      
      protected var txt:String;
      
      protected var txtColor:uint;
      
      protected var state:int = -1;
      
      protected var myWidth:int = 128;
      
      protected var _data:Object;
      
      public var onConfig:Boolean = false;
      
      public function DListItemRenderer()
      {
         super();
         var _loc1_:TextFormat = this.textField.getTextFormat();
         _loc1_.font = "tahoma";
      }
      
      public function get data() : Object
      {
         return this._data;
      }
      
      public function set data(param1:Object) : void
      {
         if(!param1)
         {
            return;
         }
         if(this._data == param1)
         {
            return;
         }
         _data = param1;
      }
      
      public function setData(param1:Object) : void
      {
         data = param1;
         this.updateValues();
      }
      
      public function initProp(param1:int) : void
      {
         this.textField.width = param1 - 2;
         this._back.width = param1;
         this._txback.width = param1;
         myWidth = param1;
      }
      
      protected function configUI() : void
      {
         onConfig = true;
      }
      
      protected function updateValues() : void
      {
         var _loc1_:TextFormat;
         var _loc2_:Boolean;
         var _loc3_:ColorTransform;
         if(this._data)
         {
            if(this._data.cur == undefined)
            {
               if(this.txAlign != this._data.textAlign)
               {
                  _loc1_ = this.textField.getTextFormat();
                  txAlign = this._data.textAlign;
                  _loc1_ = this.textField.getTextFormat();
                  _loc1_.align = "center";
                  if(this._data.textAlign == 0)
                  {
                     _loc1_.align = "left";
                  }
                  else if(this._data.textAlign == 1)
                  {
                     _loc1_.align = "center";
                  }
                  else
                  {
                     _loc1_.align = "right";
                  }
                  this.textField.setTextFormat(_loc1_);
                  this.textField.defaultTextFormat = _loc1_;
               }
               if(this._txback.alpha != this._data.backgroundAlpha)
               {
                  this._txback.alpha = this._data.backgroundAlpha;
               }
               if(this.txtColor != this._data.txColor)
               {
                  this.textField.textColor = this._data.txColor;
                  txtColor = this._data.txColor;
               }
               if(this.txt != this._data.text)
               {
                  this.textField.text = this._data.text;
                  txt = this._data.text;
               }
               if(this.state != 1)
               {
                  this._progress.visible = false;
                  this._back.visible = false;
                  this._txback.visible = true;
                  state = 1;
               }
            }
            else
            {
               if(this.state != 2)
               {
                  this._progress.visible = true;
                  this._back.visible = true;
                  this._txback.visible = false;
                  state = 2;
               }
               if(this._data.tot == undefined)
               {
                  this._data.tot = 1;
               }
               if(this._progress.alpha != this._data.progressAlpha)
               {
                  this._progress.alpha = this._data.progressAlpha;
               }
               if(this._back.alpha != this._data.backgroundAlpha)
               {
                  this._back.alpha = this._data.backgroundAlpha;
               }
               _loc2_ = false;
               if(this.pgColor != this._data.pgColor || this.bgColor != this._data.bgColor)
               {
                  _loc2_ = true;
                  pgColor = this._data.pgColor;
                  bgColor = this._data.bgColor;
               }
               if(this.txAlign != 1)
               {
                  txAlign = 1;
                  _loc1_ = this.textField.getTextFormat();
                  _loc1_.align = "center";
                  this.textField.defaultTextFormat = _loc1_;
               }
               if(this.txtColor != this._data.txColor)
               {
                  this.textField.textColor = this._data.txColor;
                  txtColor = this._data.txColor;
               }
               if(this.txt != this._data.text)
               {
                  this.textField.text = this._data.text;
                  txt = this._data.text;
               }
               this._progress.width = this.myWidth * (this._data.cur / this._data.tot);
               if(_loc2_)
               {
                  _loc3_ = this._progress.body.transform.colorTransform;
                  _loc3_.color = this.pgColor;
                  this._progress.body.transform.colorTransform = _loc3_;
                  _loc3_ = this._back.body.transform.colorTransform;
                  _loc3_.color = this.bgColor;
                  this._back.body.transform.colorTransform = _loc3_;
               }
            }
         }
      }
      
      protected function updateText() : void
      {
         if(!this._data)
         {
            return;
         }
         this.updateValues();
      }
   }
}

