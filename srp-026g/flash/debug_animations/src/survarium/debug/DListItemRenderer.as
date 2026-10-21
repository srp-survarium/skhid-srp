package survarium.debug
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import survarium.constants.TextAlign;
   
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
      
      // What the row shows: text, or a progress bar.
      private static const ROW_TEXT:int = 1;
      
      private static const ROW_PROGRESS:int = 2;
      
      protected var state:int = -1;
      
      protected var myWidth:int = 128;
      
      protected var _data:Object;
      
      public var onConfig:Boolean = false;
      
      public function DListItemRenderer()
      {
         super();
         var format:TextFormat = this.textField.getTextFormat();
         format.font = "tahoma";
      }
      
      public function get data() : Object
      {
         return this._data;
      }
      
      public function set data(value:Object) : void
      {
         if(!value)
         {
            return;
         }
         if(this._data == value)
         {
            return;
         }
         _data = value;
      }
      
      public function setData(newData:Object) : void
      {
         data = newData;
         this.updateValues();
      }
      
      public function initProp(newWidth:int) : void
      {
         this.textField.width = newWidth - 2;
         this._back.width = newWidth;
         this._txback.width = newWidth;
         myWidth = newWidth;
      }
      
      protected function configUI() : void
      {
         onConfig = true;
      }
      
      protected function updateValues() : void
      {
         var format:TextFormat;
         var colorsChanged:Boolean;
         var tint:ColorTransform;
         if(this._data)
         {
            if(this._data.cur == undefined)
            {
               if(this.txAlign != this._data.textAlign)
               {
                  format = this.textField.getTextFormat();
                  txAlign = this._data.textAlign;
                  format = this.textField.getTextFormat();
                  format.align = "center";
                  if(this._data.textAlign == TextAlign.LEFT)
                  {
                     format.align = "left";
                  }
                  else if(this._data.textAlign == TextAlign.CENTER)
                  {
                     format.align = "center";
                  }
                  else
                  {
                     format.align = "right";
                  }
                  this.textField.setTextFormat(format);
                  this.textField.defaultTextFormat = format;
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
               if(this.state != ROW_TEXT)
               {
                  this._progress.visible = false;
                  this._back.visible = false;
                  this._txback.visible = true;
                  state = ROW_TEXT;
               }
            }
            else
            {
               if(this.state != ROW_PROGRESS)
               {
                  this._progress.visible = true;
                  this._back.visible = true;
                  this._txback.visible = false;
                  state = ROW_PROGRESS;
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
               colorsChanged = false;
               if(this.pgColor != this._data.pgColor || this.bgColor != this._data.bgColor)
               {
                  colorsChanged = true;
                  pgColor = this._data.pgColor;
                  bgColor = this._data.bgColor;
               }
               if(this.txAlign != TextAlign.CENTER)
               {
                  txAlign = TextAlign.CENTER;
                  format = this.textField.getTextFormat();
                  format.align = "center";
                  this.textField.defaultTextFormat = format;
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
               if(colorsChanged)
               {
                  tint = this._progress.body.transform.colorTransform;
                  tint.color = this.pgColor;
                  this._progress.body.transform.colorTransform = tint;
                  tint = this._back.body.transform.colorTransform;
                  tint.color = this.bgColor;
                  this._back.body.transform.colorTransform = tint;
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

